"""Playable Pygame frontend for the PyO3-backed Dynamo Core."""
from __future__ import annotations

import sys
import secrets
import time
from pathlib import Path
from time import perf_counter

import pygame
from tetradynamo import Replay, ReplayRecorder, TICK_HZ

LOGICAL_SIZE = (1280, 720)
LOGICAL_WIDTH, LOGICAL_HEIGHT = LOGICAL_SIZE
BLACK, RED, WHITE = (0, 0, 0), (255, 0, 0), (255, 255, 255)
HIDDEN_ROWS, BOARD_ROWS = 2, 22
# Keep all gameplay UI relative to the centered well rather than a magic
# screen coordinate. This also keeps overlays centered over the playable area.
CELL_SIZE = 27
BOARD_WIDTH = CELL_SIZE * 10
BOARD_X, BOARD_Y = (LOGICAL_WIDTH - BOARD_WIDTH) // 2, 76
WELL_CENTER_X = BOARD_X + BOARD_WIDTH // 2
STATUS_PANEL = (BOARD_X - 420, 60, 300, 210)
NOTICE_PANEL = (BOARD_X - 420, 280, 350, 50)
HOLD_PANEL = (BOARD_X - 390, 350, 220, 175)
NEXT_PANEL = (BOARD_X + BOARD_WIDTH + 100, 60, 120, 500)
CONTROLS_PANEL = (995, 60, 270, 220)
KEY_DOWN = {
    pygame.K_LEFT: "left_press", pygame.K_a: "left_press",
    pygame.K_RIGHT: "right_press", pygame.K_d: "right_press",
    pygame.K_DOWN: "soft_drop_press", pygame.K_s: "soft_drop_press",
    pygame.K_UP: "rotate_cw", pygame.K_x: "rotate_cw", pygame.K_z: "rotate_ccw",
    pygame.K_SPACE: "hard_drop", pygame.K_c: "hold", pygame.K_h: "hold",
    pygame.K_p: "pause", pygame.K_RETURN: "start", pygame.K_r: "restart",
}
KEY_UP = {
    pygame.K_LEFT: "left_release", pygame.K_a: "left_release",
    pygame.K_RIGHT: "right_release", pygame.K_d: "right_release",
    pygame.K_DOWN: "soft_drop_release", pygame.K_s: "soft_drop_release",
}
PAUSE_OPTIONS = ("RESUME", "RESTART", "RETURN TO TITLE")
TITLE_OPTIONS = ("SOLO", "REPLAY", "CONFIG")
SOLO_OPTIONS = ("MARATHON", "SPRINT")
KONAMI = (pygame.K_UP, pygame.K_UP, pygame.K_DOWN, pygame.K_DOWN, pygame.K_LEFT, pygame.K_RIGHT, pygame.K_LEFT, pygame.K_RIGHT, pygame.K_b, pygame.K_a)


def text(surface, font, value, position, color=RED, center=False):
    image = font.render(value, True, color)
    rect = image.get_rect(center=position) if center else image.get_rect(topleft=position)
    surface.blit(image, rect)


def fragments(surface, font, values, position, center=False):
    """Draw colored text fragments as one contiguous label."""
    images = [(font.render(value, True, color), color) for value, color in values]
    width = sum(image.get_width() for image, _ in images)
    x = position[0] - width // 2 if center else position[0]
    for image, _ in images:
        surface.blit(image, (x, position[1]))
        x += image.get_width()


def panel(surface, rect):
    pygame.draw.rect(surface, RED, rect, 2)


def piece_cells(kind: str, rotation=0):
    cells = {
        "I": [(0, 1), (1, 1), (2, 1), (3, 1)],
        "O": [(1, 0), (2, 0), (1, 1), (2, 1)],
        "T": [(1, 0), (0, 1), (1, 1), (2, 1)],
        "J": [(0, 0), (0, 1), (1, 1), (2, 1)],
        "L": [(2, 0), (0, 1), (1, 1), (2, 1)],
        "S": [(1, 0), (2, 0), (0, 1), (1, 1)],
        "Z": [(0, 0), (1, 0), (1, 1), (2, 1)],
    }[kind]
    for _ in range(rotation % 4):
        cells = [(3 - y, x) for x, y in cells]
    return cells


def draw_piece(surface, kind, origin, cell, rotation=0):
    if not kind:
        return
    for x, y in piece_cells(kind, rotation):
        pygame.draw.rect(surface, RED, (origin[0] + x * cell, origin[1] + y * cell, cell - 2, cell - 2))


def visible_piece_cells(origin_x, origin_y, cells):
    """Convert core board coordinates to visible playfield coordinates."""
    return [
        (origin_x + dx, origin_y + dy - HIDDEN_ROWS)
        for dx, dy in cells
        if HIDDEN_ROWS <= origin_y + dy < BOARD_ROWS
    ]


def focus_loss_commands():
    return ["left_release", "right_release", "soft_drop_release", "pause"]


def random_seed():
    return secrets.randbits(64)


def parse_seed(value):
    try:
        seed = int(value, 0)
    except ValueError:
        return None
    return seed if 0 <= seed <= 0xFFFF_FFFF_FFFF_FFFF else None


def countdown_label(snapshot):
    phase = snapshot["phase"]
    if phase == "ready":
        return "GET READY"
    if phase in ("startcountdown", "resumecountdown"):
        return str(max(1, (snapshot["timers"]["phase_remaining_ticks"] + 999) // 1000))
    return None


def logical_mouse(position, window_size):
    width, height = window_size
    scale = min(width / LOGICAL_SIZE[0], height / LOGICAL_SIZE[1])
    return ((position[0] - (width - LOGICAL_SIZE[0] * scale) / 2) / scale,
            (position[1] - (height - LOGICAL_SIZE[1] * scale) / 2) / scale)


def pause_option_at(position):
    x, y = position
    if WELL_CENTER_X - 120 <= x <= WELL_CENTER_X + 120 and 390 <= y < 480:
        return int((y - 390) // 30)
    return None


def title_option_at(position, screen):
    x, y = position
    options = TITLE_OPTIONS if screen == "root" else SOLO_OPTIONS if screen == "solo" else ()
    if 500 <= x <= 780 and 480 <= y < 480 + len(options) * 40:
        return int((y - 480) // 40)
    return None


def clearing_cell_is_white(clearing_rows, clear_ticks, board_row, column):
    return board_row in clearing_rows and clear_ticks < (column + 1) * 25


def level_flash_visible(remaining):
    elapsed = max(0, 500 - remaining)
    return elapsed < 125 or 250 <= elapsed < 375


def chain_number(combo):
    return combo - 1 if combo >= 2 else None


def format_mode_time(ticks):
    """Format deterministic mode time (1,000 ticks per second)."""
    hours, remainder = divmod(ticks, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    seconds, milliseconds = divmod(remainder, 1_000)
    return f"{hours:02}:{minutes:02}:{seconds:02}:{milliseconds // 10:02}"


def clear_notice_fragments(notice):
    clear, points, spin, back_to_back = notice
    prefix = "B2B " if back_to_back else ""
    if spin == "mini":
        prefix += "MINI TSPIN "
    elif spin:
        prefix += "TSPIN "
    if clear == "TETRA|D":
        return [(prefix + "TETRA|", WHITE), ("D", RED), (f"  +{points}", WHITE)]
    return [(f"{prefix}{clear}  +{points}", WHITE)]


def draw_game(surface, snapshot, fonts, flash_ticks, pause_index, show_seed, mode, replay=False):
    surface.fill(BLACK)
    tiny, normal, title_font = fonts
    board_x, board_y, cell = BOARD_X, BOARD_Y, CELL_SIZE
    border = WHITE if level_flash_visible(flash_ticks) else RED
    pygame.draw.rect(surface, border, (board_x - 4, board_y - 4, cell * 10 + 8, cell * 20 + 8), 3)
    clearing_rows = snapshot["clearing_rows"]
    clear_ticks = snapshot["timers"]["clear_ticks"]
    show_well = snapshot["phase"] in ("falling", "clearing")
    if show_well:
        for y, row in enumerate(snapshot["board"][2:]):
            for x, locked in enumerate(row):
                rect = (board_x + x * cell, board_y + y * cell, cell - 2, cell - 2)
                board_row = y + HIDDEN_ROWS
                if clearing_cell_is_white(clearing_rows, clear_ticks, board_row, x):
                    pygame.draw.rect(surface, WHITE, rect)
                elif locked and board_row not in clearing_rows:
                    pygame.draw.rect(surface, RED, rect)
                else:
                    pygame.draw.rect(surface, (56, 0, 0), rect, 1)
    active = snapshot["active"]
    if snapshot["phase"] == "falling":
        for x, y in visible_piece_cells(active["x"], snapshot["ghost_y"], active["cells"]):
            pygame.draw.rect(surface, (100, 0, 0), (board_x + x * cell, board_y + y * cell, cell - 2, cell - 2), 3)
        for x, y in visible_piece_cells(active["x"], active["y"], active["cells"]):
            pygame.draw.rect(surface, RED, (board_x + x * cell, board_y + y * cell, cell - 2, cell - 2))
    for rect in (STATUS_PANEL, HOLD_PANEL, NEXT_PANEL, CONTROLS_PANEL):
        panel(surface, rect)
    elapsed = format_mode_time(snapshot["timers"]["mode_elapsed_ticks"])
    stats = [f"LINES   {snapshot['lines']}", f"LEVEL   {snapshot['level']}", f"SCORE   {snapshot['score']}", f"TIME    {elapsed}", f"GRAVITY {snapshot['gravity_cps']} cells/s"]
    text(surface, normal, "STATUS", (STATUS_PANEL[0] + 30, 82))
    for index, value in enumerate(stats):
        text(surface, tiny, value, (STATUS_PANEL[0] + 30, 115 + index * 29))
    text(surface, normal, "HOLD", (HOLD_PANEL[0] + 30, 365))
    draw_piece(surface, snapshot["hold"], (HOLD_PANEL[0] + 30, 400), 18)
    text(surface, normal, "NEXT", (NEXT_PANEL[0] + 30, 82))
    for index, kind in enumerate(snapshot["next"][:5]):
        draw_piece(surface, kind, (NEXT_PANEL[0] + 30, 115 + index * 88), 16)
    notice = snapshot["clear_notice"]
    if notice:
        panel(surface, NOTICE_PANEL)
        fragments(surface, normal, clear_notice_fragments(notice), (NOTICE_PANEL[0] + 15, 290))
    if (chain := chain_number(snapshot["combo"])) is not None:
        fragments(surface, normal, [("CHAIN|", WHITE), (str(chain), RED)], (board_x + cell * 5, 640), center=True)
    controls = [
        "ARROWS/A D  MOVE",
        "UP/X, Z     ROTATE",
        "DOWN/S      SOFT DROP",
        "SPACE       HARD DROP",
        "C/H         HOLD",
        "P           PAUSE",
        "ESC         PAUSE",
    ]
    if replay:
        controls = ["REPLAY", "P/ESC       PAUSE", "NO LIVE INPUT"]
    text(surface, normal, "CONTROLS", (1010, 75))
    for index, control in enumerate(controls):
        text(surface, tiny, control, (1010, 112 + index * 23))
    if show_seed:
        text(surface, tiny, f"SEED 0x{snapshot['seed']:X}", (24, 680), (0, 0, 255))
    phase = snapshot["phase"]
    if (countdown := countdown_label(snapshot)):
        text(surface, title_font if countdown == "GET READY" else normal, countdown, (WELL_CENTER_X, 350), WHITE, center=True)
    if phase == "paused" and not replay:
        text(surface, normal, "PAUSED", (WELL_CENTER_X, 300), WHITE, center=True)
        for index, option in enumerate(PAUSE_OPTIONS):
            color = WHITE if index == pause_index else RED
            text(surface, normal, option, (WELL_CENTER_X, 390 + index * 30), color, center=True)
    if phase in ("gameover", "complete") and not replay:
        message = {"paused": "PAUSED", "gameover": "GAME OVER — R TO RESTART", "complete": f"{mode.upper()} COMPLETE — R TO RESTART"}[phase]
        text(surface, normal, message, (WELL_CENTER_X, 370), WHITE, center=True)
        text(surface, tiny, "F5 SAVE REPLAY    ESC RETURN TO TITLE", (WELL_CENTER_X, 415), WHITE, center=True)
    if replay:
        text(surface, normal, "REPLAY", (1010, 300), WHITE)


def draw_title(surface, fonts, seed_prompt, seed_text, seed_error, title_screen, title_index, replays):
    surface.fill(BLACK)
    tiny, normal, title_font = fonts
    fragments(surface, title_font, [("TETRA|", WHITE), ("D", RED)], (640, 210), center=True)
    if title_screen in ("root", "solo"):
        options = TITLE_OPTIONS if title_screen == "root" else SOLO_OPTIONS
        if title_screen == "solo": text(surface, normal, "SOLO", (640, 440), RED, center=True)
        for index, option in enumerate(options):
            text(surface, normal, f"> {option}" if index == title_index else option, (640, 480 + index * 40), WHITE if index == title_index else RED, center=True)
        text(surface, tiny, "UP/DOWN + ENTER/SPACE", (640, 640), RED, center=True)
    elif title_screen == "replay":
        text(surface, normal, "REPLAY", (640, 440), RED, center=True)
        if not replays:
            text(surface, normal, "NO REPLAYS FOUND", (640, 500), WHITE, center=True)
        start = max(0, title_index - 3)
        for row, (name, replay, detail) in enumerate(replays[start:start + 7]):
            text(surface, tiny, f"{name}  {detail}", (300, 480 + row * 28), WHITE if start + row == title_index else RED)
    else:
        text(surface, normal, f"{title_screen.upper()} — COMING SOON", (640, 500), WHITE, center=True)
        text(surface, tiny, "ESC TO RETURN", (640, 550), RED, center=True)
    if seed_prompt:
        text(surface, normal, "ENTER SEED (DECIMAL OR 0xHEX)", (640, 300), WHITE, center=True)
        text(surface, normal, seed_text or "_", (640, 340), WHITE, center=True)
        if seed_error:
            text(surface, tiny, "INVALID UNSIGNED 64-BIT SEED", (640, 375), RED, center=True)


def main():
    pygame.init()
    window = pygame.display.set_mode((1280, 720), pygame.RESIZABLE)
    pygame.display.set_caption("Tetra|D")
    logical = pygame.Surface(LOGICAL_SIZE)
    fonts = (pygame.font.SysFont("consolas", 20, bold=True), pygame.font.SysFont("consolas", 28, bold=True), pygame.font.SysFont("consolas", 54, bold=True))
    selected_mode = "marathon"
    game = ReplayRecorder(0x544554524144, selected_mode)
    snapshot, pending = game.snapshot(), []
    pause_index, seed_locked, show_seed = 0, False, False
    title_screen, title_index = "root", 0
    seed_prompt, seed_text, seed_error, konami = False, "", False, []
    accumulator, previous, running, flash = 0.0, perf_counter(), True, 0
    replays = []
    playback = None
    playback_paused = playback_ended = False
    replay_pause_index = 0
    playback_error = None
    save_prompt, save_name, save_message = False, "", ""

    def discover_replays():
        entries = []
        for path in sorted(Path("replays").glob("*.tdr.json")):
            try:
                replay = Replay.from_json(path.read_text(encoding="utf-8"))
                meta = replay.metadata()
                detail = f"{meta['mode'].upper()}  SCORE {meta['score']}  SEED 0x{meta['seed']:X}  {meta['ticks'] // TICK_HZ}s  {meta['phase'].upper()}"
                entries.append((path.name, replay, detail))
            except (ValueError, OSError, UnicodeError) as error:
                entries.append((path.name, None, f"INVALID: {error}"))
        return entries

    def apply(result):
        nonlocal snapshot, flash
        snapshot = result["snapshot"]
        if any(item["type"] == "level_changed" for item in result["events"]):
            flash = 500

    def select_pause_option():
        nonlocal pause_index, seed_locked, show_seed, game
        if pause_index == 0:
            pending.append("resume")
        elif pause_index == 1:
            seed = snapshot["seed"] if seed_locked else random_seed()
            game = ReplayRecorder(seed, selected_mode)
            apply(game.start_with_seed(seed))
        else:
            apply(game.return_to_title())

    while running:
        now = perf_counter(); accumulator += min(now - previous, 0.25); previous = now
        for event in pygame.event.get():
            if event.type == pygame.QUIT: running = False
            elif playback is not None:
                if event.type == pygame.KEYDOWN:
                    if playback_ended and event.key == pygame.K_ESCAPE:
                        playback = None; title_screen, title_index = "root", 0; accumulator = 0
                        apply(game.return_to_title())
                    elif playback_ended and event.key == pygame.K_r:
                        playback = playback_replay.player(); snapshot = playback.snapshot()
                        playback_ended, playback_error, playback_paused, accumulator = False, None, False, 0
                    elif not playback_ended and event.key in (pygame.K_p, pygame.K_ESCAPE):
                        playback_paused = not playback_paused; replay_pause_index = 0; accumulator = 0
                    elif playback_paused and not playback_ended and event.key in (pygame.K_UP, pygame.K_DOWN):
                        replay_pause_index = 1 - replay_pause_index
                    elif playback_paused and not playback_ended and event.key in (pygame.K_RETURN, pygame.K_SPACE):
                        if replay_pause_index == 0:
                            playback_paused = False; accumulator = 0
                        else:
                            playback = None; playback_paused = False; title_screen, title_index = "root", 0; accumulator = 0
                            apply(game.return_to_title())
                elif playback_paused and not playback_ended and event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN):
                    option = pause_option_at(logical_mouse(event.pos, window.get_size()))
                    if option is not None and option < 2:
                        replay_pause_index = option
                        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                            if option == 0:
                                playback_paused = False; accumulator = 0
                            else:
                                playback = None; playback_paused = False; title_screen, title_index = "root", 0; accumulator = 0
                                apply(game.return_to_title())
                continue
            elif event.type == pygame.KEYDOWN:
                if save_prompt:
                    if event.key == pygame.K_ESCAPE: save_prompt = False
                    elif event.key == pygame.K_BACKSPACE: save_name = save_name[:-1]
                    elif event.key == pygame.K_RETURN:
                        try:
                            Path("replays").mkdir(exist_ok=True)
                            path = Path("replays") / f"{save_name or selected_mode}-{int(time.time())}.tdr.json"
                            path.write_text(game.replay().to_json(), encoding="utf-8")
                            save_message = f"REPLAY SAVED: {path}"
                        except (OSError, ValueError) as error: save_message = f"SAVE FAILED: {error}"
                        save_prompt = False
                    elif event.unicode and event.unicode.isascii() and (event.unicode.isalnum() or event.unicode in "_-") and len(save_name) < 48:
                        save_name += event.unicode
                    continue
                if seed_prompt:
                    if event.key == pygame.K_ESCAPE:
                        seed_prompt, seed_error = False, False
                    elif event.key == pygame.K_BACKSPACE:
                        seed_text = seed_text[:-1]
                    elif event.key == pygame.K_RETURN:
                        seed = parse_seed(seed_text)
                        if seed is None:
                            seed_error = True
                        else:
                            seed_locked, show_seed, seed_prompt, seed_error = True, True, False, False
                            apply(game.start_with_seed(seed))
                    elif event.unicode and event.unicode in "0123456789abcdefABCDEFxX":
                        seed_text += event.unicode
                    continue
                if snapshot["phase"] in ("title", "paused"):
                    konami = (konami + [event.key])[-len(KONAMI):]
                    if tuple(konami) == KONAMI:
                        konami = []
                        if snapshot["phase"] == "title" and title_screen == "root":
                            seed_prompt, seed_text, seed_error = True, "", False
                        else:
                            seed_locked = not seed_locked
                            show_seed = seed_locked
                        continue
                if snapshot["phase"] in ("gameover", "complete") and event.key == pygame.K_F5:
                    save_prompt, save_name, save_message = True, "", ""
                elif snapshot["phase"] in ("gameover", "complete") and event.key == pygame.K_ESCAPE:
                    apply(game.return_to_title()); title_screen, title_index = "root", 0
                elif snapshot["phase"] == "title" and event.key == pygame.K_ESCAPE and title_screen != "root":
                    title_screen, title_index = "root", 0
                elif event.key == pygame.K_ESCAPE or event.key == pygame.K_p:
                    if snapshot["phase"] == "paused":
                        pending.append("resume")
                    elif snapshot["phase"] in ("falling", "clearing"):
                        pending.append("pause")
                elif snapshot["phase"] == "title":
                    options = TITLE_OPTIONS if title_screen == "root" else SOLO_OPTIONS if title_screen == "solo" else replays if title_screen == "replay" else ()
                    if event.key == pygame.K_ESCAPE and title_screen != "root":
                        title_screen, title_index = "root", 0
                    elif event.key == pygame.K_UP and options:
                        title_index = (title_index - 1) % len(options)
                    elif event.key == pygame.K_DOWN and options:
                        title_index = (title_index + 1) % len(options)
                    elif event.key in (pygame.K_RETURN, pygame.K_SPACE):
                        if title_screen == "root":
                            selected = TITLE_OPTIONS[title_index]
                            title_screen, title_index = ("solo", 0) if selected == "SOLO" else (selected.lower(), 0)
                            if title_screen == "replay": replays = discover_replays()
                        elif title_screen == "solo":
                            selected_mode = SOLO_OPTIONS[title_index].lower()
                            seed = snapshot["seed"] if seed_locked else random_seed()
                            game = ReplayRecorder(seed, selected_mode)
                            apply(game.start_with_seed(seed))
                        elif title_screen == "replay" and replays and replays[title_index][1]:
                            playback_replay = replays[title_index][1]; playback = playback_replay.player()
                            snapshot = playback.snapshot(); playback_ended = playback_paused = False; playback_error = None; pending.clear(); accumulator = 0
                elif snapshot["phase"] == "paused":
                    if event.key == pygame.K_UP:
                        pause_index = (pause_index - 1) % len(PAUSE_OPTIONS)
                    elif event.key == pygame.K_DOWN:
                        pause_index = (pause_index + 1) % len(PAUSE_OPTIONS)
                    elif event.key in (pygame.K_SPACE, pygame.K_RETURN):
                        select_pause_option()
                elif snapshot["phase"] in ("gameover", "complete") and event.key == pygame.K_r:
                    seed = snapshot["seed"] if seed_locked else random_seed()
                    game = ReplayRecorder(seed, selected_mode)
                    apply(game.start_with_seed(seed))
                elif event.key in KEY_DOWN:
                    pending.append(KEY_DOWN[event.key])
            elif event.type == pygame.KEYUP and event.key in KEY_UP: pending.append(KEY_UP[event.key])
            elif event.type == pygame.WINDOWFOCUSLOST:
                pending.extend(focus_loss_commands())
            elif event.type == pygame.MOUSEMOTION and snapshot["phase"] == "paused":
                option = pause_option_at(logical_mouse(event.pos, window.get_size()))
                if option is not None:
                    pause_index = option
            elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 1 and snapshot["phase"] == "paused":
                option = pause_option_at(logical_mouse(event.pos, window.get_size()))
                if option is not None:
                    pause_index = option
                    select_pause_option()
            elif event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN) and snapshot["phase"] == "title" and not seed_prompt:
                mouse = logical_mouse(event.pos, window.get_size())
                if title_screen == "replay" and 300 <= mouse[0] <= 1100 and 480 <= mouse[1] < 676:
                    option = max(0, title_index - 3) + int((mouse[1] - 480) // 28)
                    if option >= len(replays): option = None
                else:
                    option = title_option_at(mouse, title_screen)
                if option is not None:
                    title_index = option
                    if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                        if title_screen == "root":
                            selected = TITLE_OPTIONS[title_index]
                            title_screen, title_index = ("solo", 0) if selected == "SOLO" else (selected.lower(), 0)
                            if title_screen == "replay": replays = discover_replays()
                        elif title_screen == "solo":
                            selected_mode = SOLO_OPTIONS[title_index].lower()
                            seed = snapshot["seed"] if seed_locked else random_seed()
                            game = ReplayRecorder(seed, selected_mode)
                            apply(game.start_with_seed(seed))
                        elif title_screen == "replay" and replays[title_index][1]:
                            playback_replay = replays[title_index][1]; playback = playback_replay.player()
                            snapshot = playback.snapshot(); playback_ended = playback_paused = False; playback_error = None; pending.clear(); accumulator = 0
        if playback is not None:
            while not playback_paused and not playback_ended and accumulator >= 1 / TICK_HZ:
                try:
                    result = playback.advance()
                    if result is None:
                        playback.finish(); snapshot = playback.snapshot(); playback_ended = True; break
                    snapshot = result["snapshot"]
                    if any(item["type"] == "level_changed" for item in result["events"]): flash = 500
                except ValueError as error:
                    playback_error = str(error); playback_ended = True; break
                flash = max(0, flash - 1); accumulator -= 1 / TICK_HZ
            if playback_paused or playback_ended: accumulator = 0
        if playback is None and snapshot["phase"] == "paused" and "resume" not in pending:
            accumulator = 0
        while playback is None and accumulator >= 1 / TICK_HZ:
            result = game.step(pending); pending = []; snapshot = result["snapshot"]
            if any(event["type"] == "level_changed" for event in result["events"]): flash = 500
            flash = max(0, flash - 1); accumulator -= 1 / TICK_HZ
        if playback is None and snapshot["phase"] == "title": draw_title(logical, fonts, seed_prompt, seed_text, seed_error, title_screen, title_index, replays)
        else:
            draw_game(logical, snapshot, fonts, flash, pause_index, show_seed, playback_replay.metadata()["mode"] if playback else selected_mode, playback is not None)
            if playback_paused:
                text(logical, fonts[1], "REPLAY PAUSED", (WELL_CENTER_X, 300), WHITE, center=True)
                for index, option in enumerate(("RESUME", "RETURN TO TITLE")):
                    text(logical, fonts[1], option, (WELL_CENTER_X, 390 + index * 30), WHITE if index == replay_pause_index else RED, center=True)
            if playback_ended:
                text(logical, fonts[1], "REPLAY ERROR" if playback_error else "REPLAY ENDED", (WELL_CENTER_X, 350), WHITE, center=True)
                text(logical, fonts[0], playback_error or "R REPLAY AGAIN   ESC RETURN TO TITLE", (WELL_CENTER_X, 390), WHITE, center=True)
            if save_prompt:
                text(logical, fonts[1], "SAVE REPLAY AS", (WELL_CENTER_X, 465), WHITE, center=True)
                text(logical, fonts[1], save_name or selected_mode, (WELL_CENTER_X, 500), WHITE, center=True)
            elif save_message: text(logical, fonts[0], save_message, (WELL_CENTER_X, 500), WHITE, center=True)
        width, height = window.get_size(); scale = min(width / LOGICAL_SIZE[0], height / LOGICAL_SIZE[1])
        scaled = pygame.transform.smoothscale(logical, (int(LOGICAL_SIZE[0] * scale), int(LOGICAL_SIZE[1] * scale)))
        window.fill(BLACK); window.blit(scaled, ((width - scaled.get_width()) // 2, (height - scaled.get_height()) // 2)); pygame.display.flip()
    pygame.quit()


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    main()
