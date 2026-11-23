"""Playable Pygame frontend for the PyO3-backed Dynamo Core."""
from __future__ import annotations

import sys
from pathlib import Path
from time import perf_counter

import pygame
from tetradynamo import Game, TICK_HZ

LOGICAL_SIZE = (1280, 720)
BLACK, RED, WHITE = (0, 0, 0), (255, 0, 0), (255, 255, 255)
HIDDEN_ROWS, BOARD_ROWS = 2, 22
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


def clearing_cell_is_white(clearing_rows, clear_ticks, board_row, column):
    return board_row in clearing_rows and clear_ticks < (column + 1) * 25


def level_flash_visible(remaining):
    elapsed = max(0, 500 - remaining)
    return elapsed < 125 or 250 <= elapsed < 375


def chain_number(combo):
    return combo - 1 if combo >= 2 else None


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


def draw_game(surface, snapshot, fonts, flash_ticks):
    surface.fill(BLACK)
    tiny, normal, title_font = fonts
    board_x, board_y, cell = 480, 76, 27
    border = WHITE if level_flash_visible(flash_ticks) else RED
    pygame.draw.rect(surface, border, (board_x - 4, board_y - 4, cell * 10 + 8, cell * 20 + 8), 3)
    clearing_rows = snapshot["clearing_rows"]
    clear_ticks = snapshot["timers"]["clear_ticks"]
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
    elapsed = snapshot["timers"]["active_gameplay_ticks"] // TICK_HZ
    stats = [f"LINES   {snapshot['lines']}", f"LEVEL   {snapshot['level']}", f"SCORE   {snapshot['score']}", f"TIME    {elapsed // 60:02}:{elapsed % 60:02}", f"GRAVITY {snapshot['gravity_cps']} cells/s"]
    text(surface, normal, "STATUS", (90, 90))
    for index, value in enumerate(stats):
        text(surface, tiny, value, (90, 125 + index * 31))
    text(surface, normal, "HOLD", (120, 330)); draw_piece(surface, snapshot["hold"], (120, 365), 18)
    text(surface, normal, "NEXT", (880, 82))
    for index, kind in enumerate(snapshot["next"][:5]):
        draw_piece(surface, kind, (880, 115 + index * 88), 16)
    notice = snapshot["clear_notice"]
    if notice:
        fragments(surface, normal, clear_notice_fragments(notice), (90, 270))
    if (chain := chain_number(snapshot["combo"])) is not None:
        fragments(surface, normal, [("CHAIN|", WHITE), (str(chain), RED)], (board_x + cell * 5, 640), center=True)
    text(surface, tiny, "ARROWS/A D  MOVE     UP/X, Z  ROTATE     DOWN/S  SOFT DROP", (760, 585))
    text(surface, tiny, "SPACE  HARD DROP     C/H  HOLD     P  PAUSE     ESC  QUIT", (760, 615))
    phase = snapshot["phase"]
    if phase in ("paused", "gameover", "complete"):
        message = {"paused": "PAUSED", "gameover": "GAME OVER — R TO RESTART", "complete": "MARATHON COMPLETE — R TO RESTART"}[phase]
        text(surface, normal, message, (640, 370), WHITE, center=True)


def draw_title(surface, fonts):
    surface.fill(BLACK)
    _, normal, title_font = fonts
    fragments(surface, title_font, [("TETRA|", WHITE), ("D", RED)], (640, 210), center=True)
    text(surface, normal, "GAME MODE:", (640, 480), RED, center=True)
    text(surface, normal, "> MARATHON", (640, 520), RED, center=True)
    text(surface, normal, "PRESS ENTER TO START", (640, 600), RED, center=True)


def main():
    pygame.init()
    window = pygame.display.set_mode((1280, 720), pygame.RESIZABLE)
    pygame.display.set_caption("Tetra|D")
    logical = pygame.Surface(LOGICAL_SIZE)
    fonts = (pygame.font.SysFont("consolas", 20, bold=True), pygame.font.SysFont("consolas", 28, bold=True), pygame.font.SysFont("consolas", 54, bold=True))
    game = Game(0x544554524144)
    snapshot, pending = game.snapshot(), []
    accumulator, previous, running, flash = 0.0, perf_counter(), True, 0
    while running:
        now = perf_counter(); accumulator += min(now - previous, 0.25); previous = now
        for event in pygame.event.get():
            if event.type == pygame.QUIT: running = False
            elif event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE: running = False
                elif event.key == pygame.K_p:
                    pending.append("resume" if snapshot["phase"] == "paused" else "pause")
                elif event.key in KEY_DOWN: pending.append(KEY_DOWN[event.key])
            elif event.type == pygame.KEYUP and event.key in KEY_UP: pending.append(KEY_UP[event.key])
            elif event.type == pygame.WINDOWFOCUSLOST:
                pending.extend(focus_loss_commands())
        while accumulator >= 1 / TICK_HZ:
            result = game.step(pending); pending = []; snapshot = result["snapshot"]
            if any(event["type"] == "level_changed" for event in result["events"]): flash = 500
            flash = max(0, flash - 1); accumulator -= 1 / TICK_HZ
        if snapshot["phase"] == "title": draw_title(logical, fonts)
        else: draw_game(logical, snapshot, fonts, flash)
        width, height = window.get_size(); scale = min(width / LOGICAL_SIZE[0], height / LOGICAL_SIZE[1])
        scaled = pygame.transform.smoothscale(logical, (int(LOGICAL_SIZE[0] * scale), int(LOGICAL_SIZE[1] * scale)))
        window.fill(BLACK); window.blit(scaled, ((width - scaled.get_width()) // 2, (height - scaled.get_height()) // 2)); pygame.display.flip()
    pygame.quit()


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    main()
