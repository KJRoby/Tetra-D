//! Native Macroquad frontend. Dynamo Core remains the sole gameplay authority.

use macroquad::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, mem};
use tetradynamo_core::{
    shape_cells, Clear, ClearNotice, Command, GameConfig, GameEvent, GameSnapshot, Phase,
    PieceKind, Replay, ReplayPlayer, ReplayRecorder, Spin, HEIGHT, HIDDEN_ROWS, TICK_HZ,
};

const LOGICAL_WIDTH: f32 = 1280.0;
const LOGICAL_HEIGHT: f32 = 720.0;
const CELL_SIZE: f32 = 27.0;
const BOARD_WIDTH: f32 = CELL_SIZE * 10.0;
const BOARD_X: f32 = (LOGICAL_WIDTH - BOARD_WIDTH) / 2.0;
const BOARD_Y: f32 = 76.0;
const WELL_CENTER_X: f32 = BOARD_X + BOARD_WIDTH / 2.0;
const STATUS_X: f32 = BOARD_X - 420.0;
const NOTICE_X: f32 = BOARD_X - 420.0;
const HOLD_X: f32 = BOARD_X - 390.0;
const NEXT_X: f32 = BOARD_X + BOARD_WIDTH + 100.0;
const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const DIM_RED: Color = Color::new(0.22, 0.0, 0.0, 1.0);
const GHOST_RED: Color = Color::new(0.39, 0.0, 0.0, 1.0);

fn window_conf() -> Conf {
    Conf {
        window_title: "Tetra|D".to_owned(),
        window_width: LOGICAL_WIDTH as i32,
        window_height: LOGICAL_HEIGHT as i32,
        window_resizable: true,
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
struct View {
    scale: f32,
    x: f32,
    y: f32,
}

impl View {
    fn current() -> Self {
        let scale = (screen_width() / LOGICAL_WIDTH).min(screen_height() / LOGICAL_HEIGHT);
        Self {
            scale,
            x: (screen_width() - LOGICAL_WIDTH * scale) / 2.0,
            y: (screen_height() - LOGICAL_HEIGHT * scale) / 2.0,
        }
    }
    fn point(self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y + y * self.scale)
    }
    fn rect(self, x: f32, y: f32, width: f32, height: f32) -> (f32, f32, f32, f32) {
        let (x, y) = self.point(x, y);
        (x, y, width * self.scale, height * self.scale)
    }
    fn logical(self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.x) / self.scale, (y - self.y) / self.scale)
    }
}

#[derive(Default)]
struct HeldInput {
    left: bool,
    right: bool,
    soft: bool,
}

#[derive(Default)]
struct SeedControl {
    locked: bool,
    show: bool,
    prompt: bool,
    value: String,
    error: bool,
    code: Vec<KeyCode>,
}

struct SaveOverlay<'a> {
    prompt: bool,
    name: &'a str,
    message: Option<&'a str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleScreen {
    Root,
    Solo,
    Replay,
    Config,
}

struct ReplayEntry {
    name: String,
    replay: Result<Replay, String>,
}
struct Playback {
    replay: Replay,
    player: ReplayPlayer,
    snapshot: GameSnapshot,
    paused: bool,
    ended: bool,
    error: Option<String>,
}

impl HeldInput {
    fn reconcile(&mut self, pending: &mut Vec<Command>) {
        reconcile_key(
            &mut self.left,
            is_key_down(KeyCode::Left) || is_key_down(KeyCode::A),
            Command::LeftPress,
            Command::LeftRelease,
            pending,
        );
        reconcile_key(
            &mut self.right,
            is_key_down(KeyCode::Right) || is_key_down(KeyCode::D),
            Command::RightPress,
            Command::RightRelease,
            pending,
        );
        reconcile_key(
            &mut self.soft,
            is_key_down(KeyCode::Down) || is_key_down(KeyCode::S),
            Command::SoftDropPress,
            Command::SoftDropRelease,
            pending,
        );
    }
    fn release_all(&mut self, pending: &mut Vec<Command>) {
        if self.left {
            pending.push(Command::LeftRelease);
        }
        if self.right {
            pending.push(Command::RightRelease);
        }
        if self.soft {
            pending.push(Command::SoftDropRelease);
        }
        *self = Self::default();
    }
}

fn reconcile_key(
    known_down: &mut bool,
    down: bool,
    press: Command,
    release: Command,
    pending: &mut Vec<Command>,
) {
    if down && !*known_down {
        pending.push(press);
    } else if !down && *known_down {
        pending.push(release);
    }
    *known_down = down;
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = new_recording(0x5445_5452_4144);
    let mut snapshot = game.snapshot();
    let mut held = HeldInput::default();
    let mut pending = Vec::new();
    let mut pause_index = 0_u8;
    let mut seed_control = SeedControl::default();
    let mut accumulator = 0.0_f32;
    let mut level_flash_ticks = 0_u32;
    let mut save_prompt = false;
    let mut save_name = String::new();
    let mut save_message: Option<(String, f64)> = None;
    let mut title_screen = TitleScreen::Root;
    let mut title_index = 0_u8;
    let mut replay_entries = Vec::new();
    let mut playback: Option<Playback> = None;

    loop {
        if let Some(active) = playback.as_mut() {
            held = HeldInput::default();
            pending.clear();
            if active.ended {
                if is_key_pressed(KeyCode::R) {
                    match active.replay.player() {
                        Ok(player) => {
                            active.snapshot = player.game().snapshot();
                            active.player = player;
                            active.ended = false;
                            active.error = None;
                            accumulator = 0.0;
                        }
                        Err(error) => active.error = Some(error.to_string()),
                    }
                }
                if is_key_pressed(KeyCode::Escape) {
                    playback = None;
                    title_screen = TitleScreen::Root;
                    title_index = 0;
                    accumulator = 0.0;
                    continue;
                }
            } else if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
                active.paused = !active.paused;
                accumulator = 0.0;
            }
            if !active.paused && !active.ended {
                accumulator += get_frame_time().min(0.25);
                while accumulator >= 1.0 / TICK_HZ as f32 {
                    match active.player.advance() {
                        Ok(Some(result)) => {
                            if result
                                .events
                                .iter()
                                .any(|event| matches!(event, GameEvent::LevelChanged { .. }))
                            {
                                level_flash_ticks = 500;
                            }
                            active.snapshot = active.player.game().snapshot();
                        }
                        Ok(None) => {
                            active.ended = true;
                            if let Err(error) = active.player.finish() {
                                active.error = Some(error.to_string());
                            }
                            break;
                        }
                        Err(error) => {
                            active.ended = true;
                            active.error = Some(error.to_string());
                            break;
                        }
                    }
                    level_flash_ticks = level_flash_ticks.saturating_sub(1);
                    accumulator -= 1.0 / TICK_HZ as f32;
                }
            }
            clear_background(BLACK);
            draw_game(
                View::current(),
                &active.snapshot,
                level_flash_ticks,
                0,
                false,
                SaveOverlay {
                    prompt: false,
                    name: "",
                    message: None,
                },
                true,
            );
            if active.paused {
                centered_text(
                    View::current(),
                    "REPLAY PAUSED",
                    WELL_CENTER_X,
                    300.0,
                    28.0,
                    WHITE,
                );
                centered_text(View::current(), "RESUME", WELL_CENTER_X, 390.0, 28.0, WHITE);
            }
            if active.ended {
                centered_text(
                    View::current(),
                    if active.error.is_some() {
                        "REPLAY ERROR"
                    } else {
                        "REPLAY ENDED"
                    },
                    WELL_CENTER_X,
                    350.0,
                    28.0,
                    WHITE,
                );
                centered_text(
                    View::current(),
                    active
                        .error
                        .as_deref()
                        .unwrap_or("R — REPLAY AGAIN   ESC — RETURN TO TITLE"),
                    WELL_CENTER_X,
                    390.0,
                    16.0,
                    RED,
                );
            }
            next_frame().await;
            continue;
        }
        if save_prompt {
            if is_key_pressed(KeyCode::Escape) {
                save_prompt = false;
            } else if is_key_pressed(KeyCode::Backspace) {
                save_name.pop();
            } else if is_key_pressed(KeyCode::Enter) {
                let name = if save_name.is_empty() {
                    "marathon"
                } else {
                    &save_name
                };
                save_message = Some((save_replay(&game, name), get_time()));
                save_prompt = false;
            } else {
                while let Some(character) = get_char_pressed() {
                    if save_name_character_allowed(character) && save_name.len() < 48 {
                        save_name.push(character);
                    }
                }
            }
        } else if matches!(snapshot.phase, Phase::GameOver | Phase::Complete)
            && is_key_pressed(KeyCode::F5)
        {
            save_prompt = true;
            save_name.clear();
            drain_text_input();
        }
        if !save_prompt
            && matches!(snapshot.phase, Phase::Title | Phase::Paused)
            && !seed_control.prompt
        {
            if let Some(key) = konami_key() {
                seed_control.code.push(key);
                seed_control.code = seed_control
                    .code
                    .into_iter()
                    .rev()
                    .take(10)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if seed_control.code == konami_code() {
                    seed_control.code.clear();
                    if snapshot.phase == Phase::Title {
                        seed_control.prompt = true;
                        seed_control.value.clear();
                        seed_control.error = false;
                    } else {
                        seed_control.locked = !seed_control.locked;
                        seed_control.show = seed_control.locked;
                    }
                }
            }
        }
        if !save_prompt && seed_control.prompt {
            seed_prompt_input(&mut game, &mut snapshot, &mut seed_control);
        } else if !save_prompt && snapshot.phase == Phase::Title {
            let count = if title_screen == TitleScreen::Root {
                3
            } else if title_screen == TitleScreen::Solo {
                1
            } else if title_screen == TitleScreen::Replay {
                replay_entries.len().min(u8::MAX as usize) as u8
            } else {
                0
            };
            if is_key_pressed(KeyCode::Escape) && title_screen != TitleScreen::Root {
                title_screen = TitleScreen::Root;
                title_index = 0;
            } else if count > 0 && is_key_pressed(KeyCode::Up) {
                title_index = (title_index + count - 1) % count;
            } else if count > 0 && is_key_pressed(KeyCode::Down) {
                title_index = (title_index + 1) % count;
            } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                match title_screen {
                    TitleScreen::Root => {
                        title_screen = match title_index {
                            0 => TitleScreen::Solo,
                            1 => {
                                replay_entries = discover_replays();
                                TitleScreen::Replay
                            }
                            _ => TitleScreen::Config,
                        };
                        title_index = 0;
                    }
                    TitleScreen::Solo => {
                        let seed = if seed_control.locked {
                            snapshot.seed
                        } else {
                            random_seed()
                        };
                        game = new_recording(seed);
                        snapshot = game.start_with_seed(seed).snapshot;
                    }
                    TitleScreen::Replay => {
                        if let Some(entry) = replay_entries.get(title_index as usize) {
                            if let Ok(replay) = &entry.replay {
                                match replay.player() {
                                    Ok(player) => {
                                        let replay = replay.clone();
                                        let snapshot = player.game().snapshot();
                                        playback = Some(Playback {
                                            replay,
                                            player,
                                            snapshot,
                                            paused: false,
                                            ended: false,
                                            error: None,
                                        });
                                        accumulator = 0.0;
                                    }
                                    Err(error) => {
                                        save_message = Some((error.to_string(), get_time()))
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else if !save_prompt && matches!(snapshot.phase, Phase::Falling | Phase::Clearing) {
            held.reconcile(&mut pending);
        } else {
            held.release_all(&mut pending);
        }
        if !save_prompt && !seed_control.prompt && snapshot.phase != Phase::Title {
            queue_actions(&snapshot, &mut held, &mut pending, &mut pause_index);
        }
        if pending.contains(&Command::Restart) {
            pending.retain(|command| *command != Command::Restart);
            let seed = if seed_control.locked {
                snapshot.seed
            } else {
                random_seed()
            };
            game = new_recording(seed);
            snapshot = game.start_with_seed(seed).snapshot;
        }
        if pending.contains(&Command::ReturnToTitle) {
            // The paused replay timeline is frozen, so title navigation must
            // be applied immediately rather than waiting for another tick.
            pending.clear();
            snapshot = game.return_to_title().snapshot;
        }
        if snapshot.phase == Phase::Paused {
            let view = View::current();
            let (x, y) = view.logical(mouse_position().0, mouse_position().1);
            if ((WELL_CENTER_X - 120.0)..=(WELL_CENTER_X + 120.0)).contains(&x)
                && (390.0..480.0).contains(&y)
            {
                pause_index = ((y - 390.0) / 30.0) as u8;
                if is_mouse_button_pressed(MouseButton::Left) {
                    match pause_index {
                        0 => pending.push(Command::Resume),
                        1 => {
                            let seed = if seed_control.locked {
                                snapshot.seed
                            } else {
                                random_seed()
                            };
                            game = new_recording(seed);
                            snapshot = game.start_with_seed(seed).snapshot;
                        }
                        _ => snapshot = game.return_to_title().snapshot,
                    }
                }
            }
        }
        if snapshot.phase == Phase::Title && !seed_control.prompt {
            let view = View::current();
            let (x, y) = view.logical(mouse_position().0, mouse_position().1);
            let count = if title_screen == TitleScreen::Root {
                3
            } else if title_screen == TitleScreen::Solo {
                1
            } else if title_screen == TitleScreen::Replay {
                replay_entries.len().min(u8::MAX as usize) as u8
            } else {
                0
            };
            if (500.0..=780.0).contains(&x) && (480.0..(480.0 + count as f32 * 40.0)).contains(&y) {
                title_index = ((y - 480.0) / 40.0) as u8;
                if is_mouse_button_pressed(MouseButton::Left) {
                    match title_screen {
                        TitleScreen::Root => {
                            title_screen = match title_index {
                                0 => TitleScreen::Solo,
                                1 => {
                                    replay_entries = discover_replays();
                                    TitleScreen::Replay
                                }
                                _ => TitleScreen::Config,
                            };
                            title_index = 0;
                        }
                        TitleScreen::Solo => {
                            let seed = if seed_control.locked {
                                snapshot.seed
                            } else {
                                random_seed()
                            };
                            game = new_recording(seed);
                            snapshot = game.start_with_seed(seed).snapshot;
                        }
                        TitleScreen::Replay => {
                            if let Some(entry) = replay_entries.get(title_index as usize) {
                                if let Ok(replay) = &entry.replay {
                                    if let Ok(player) = replay.player() {
                                        let replay = replay.clone();
                                        let snapshot = player.game().snapshot();
                                        playback = Some(Playback {
                                            replay,
                                            player,
                                            snapshot,
                                            paused: false,
                                            ended: false,
                                            error: None,
                                        });
                                        accumulator = 0.0;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        accumulator += get_frame_time().min(0.25);
        if snapshot.phase == Phase::Paused && !pending.contains(&Command::Resume) {
            accumulator = 0.0;
        }
        while accumulator >= 1.0 / TICK_HZ as f32 {
            if snapshot.phase == Phase::Paused && !pending.contains(&Command::Resume) {
                accumulator = 0.0;
                break;
            }
            let commands = mem::take(&mut pending);
            let result = game
                .step(&commands)
                .expect("Macroquad input batch is bounded");
            if result
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LevelChanged { .. }))
            {
                level_flash_ticks = 500;
            }
            snapshot = result.snapshot;
            level_flash_ticks = level_flash_ticks.saturating_sub(1);
            accumulator -= 1.0 / TICK_HZ as f32;
        }

        let view = View::current();
        clear_background(BLACK);
        if snapshot.phase == Phase::Title {
            draw_title(
                view,
                &seed_control,
                title_screen,
                title_index,
                &replay_entries,
            );
        } else {
            draw_game(
                view,
                &snapshot,
                level_flash_ticks,
                pause_index,
                seed_control.show,
                SaveOverlay {
                    prompt: save_prompt,
                    name: &save_name,
                    message: save_message
                        .as_ref()
                        .filter(|(_, since)| get_time() - *since < 3.0)
                        .map(|(message, _)| message.as_str()),
                },
                false,
            );
        }
        next_frame().await;
    }
}

fn queue_actions(
    snapshot: &GameSnapshot,
    held: &mut HeldInput,
    pending: &mut Vec<Command>,
    pause_index: &mut u8,
) {
    if snapshot.phase == Phase::Paused {
        if is_key_pressed(KeyCode::Up) {
            *pause_index = (*pause_index + 2) % 3;
        }
        if is_key_pressed(KeyCode::Down) {
            *pause_index = (*pause_index + 1) % 3;
        }
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
            pending.push(match *pause_index {
                0 => Command::Resume,
                1 => Command::Restart,
                _ => Command::ReturnToTitle,
            });
        }
        if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
            pending.push(Command::Resume);
        }
        return;
    }
    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::X) {
        pending.push(Command::RotateCw);
    }
    if is_key_pressed(KeyCode::Z) {
        pending.push(Command::RotateCcw);
    }
    if is_key_pressed(KeyCode::Space) {
        pending.push(Command::HardDrop);
    }
    if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::H) {
        pending.push(Command::Hold);
    }
    if is_key_pressed(KeyCode::Enter) {
        pending.push(Command::Start);
    }
    if is_key_pressed(KeyCode::R) {
        pending.push(Command::Restart);
    }
    if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::Escape) {
        held.release_all(pending);
        pending.push(if snapshot.phase == Phase::Paused {
            Command::Resume
        } else {
            Command::Pause
        });
    }
}

fn rect(view: View, x: f32, y: f32, width: f32, height: f32, color: Color) {
    let (x, y, width, height) = view.rect(x, y, width, height);
    draw_rectangle(x, y, width, height, color);
}

fn outline(view: View, x: f32, y: f32, width: f32, height: f32, thickness: f32, color: Color) {
    let (x, y, width, height) = view.rect(x, y, width, height);
    draw_rectangle_lines(x, y, width, height, thickness * view.scale, color);
}

fn text(view: View, value: &str, x: f32, y: f32, size: f32, color: Color) {
    let (x, y) = view.point(x, y + size);
    draw_text(value, x, y, size * view.scale, color);
}

fn centered_text(view: View, value: &str, x: f32, y: f32, size: f32, color: Color) {
    let width = measure_text(value, None, size as u16, 1.0).width;
    text(view, value, x - width / 2.0, y, size, color);
}

fn fragments(view: View, values: &[(&str, Color)], x: f32, y: f32, size: f32, center: bool) {
    let width: f32 = values
        .iter()
        .map(|(value, _)| measure_text(value, None, size as u16, 1.0).width)
        .sum();
    let mut x = if center { x - width / 2.0 } else { x };
    for (value, color) in values {
        text(view, value, x, y, size, *color);
        x += measure_text(value, None, size as u16, 1.0).width;
    }
}

fn draw_game(
    view: View,
    snapshot: &GameSnapshot,
    flash_ticks: u32,
    pause_index: u8,
    show_seed: bool,
    save: SaveOverlay<'_>,
    replay_playback: bool,
) {
    let (board_x, board_y, cell) = (BOARD_X, BOARD_Y, CELL_SIZE);
    let border = if level_flash_visible(flash_ticks) {
        WHITE
    } else {
        RED
    };
    outline(
        view,
        board_x - 4.0,
        board_y - 4.0,
        278.0,
        548.0,
        3.0,
        border,
    );

    let show_well = matches!(snapshot.phase, Phase::Falling | Phase::Clearing);
    for (visible_y, row) in snapshot.board[HIDDEN_ROWS as usize..].iter().enumerate() {
        for (x, locked) in row.iter().enumerate() {
            let board_row = visible_y as u8 + HIDDEN_ROWS as u8;
            let x = x as f32;
            let y = visible_y as f32;
            if !show_well {
                continue;
            } else if clearing_cell_is_white(
                &snapshot.clearing_rows,
                snapshot.timers.clear_ticks,
                board_row,
                x as u8,
            ) {
                rect(
                    view,
                    board_x + x * cell,
                    board_y + y * cell,
                    cell - 2.0,
                    cell - 2.0,
                    WHITE,
                );
            } else if locked.is_some() && !snapshot.clearing_rows.contains(&board_row) {
                rect(
                    view,
                    board_x + x * cell,
                    board_y + y * cell,
                    cell - 2.0,
                    cell - 2.0,
                    RED,
                );
            } else {
                outline(
                    view,
                    board_x + x * cell,
                    board_y + y * cell,
                    cell - 2.0,
                    cell - 2.0,
                    1.0,
                    DIM_RED,
                );
            }
        }
    }

    if snapshot.phase == Phase::Falling {
        draw_active(view, snapshot, board_x, board_y, cell, true);
        draw_active(view, snapshot, board_x, board_y, cell, false);
    }

    for panel in [
        (STATUS_X, 60.0, 300.0, 210.0),
        (HOLD_X, 350.0, 220.0, 175.0),
        (NEXT_X, 60.0, 120.0, 500.0),
        (995.0, 60.0, 270.0, 220.0),
    ] {
        outline(view, panel.0, panel.1, panel.2, panel.3, 2.0, RED);
    }

    let elapsed = snapshot.timers.active_gameplay_ticks / TICK_HZ as u64;
    text(view, "STATUS", STATUS_X + 30.0, 82.0, 28.0, RED);
    for (index, value) in [
        format!("LINES   {}", snapshot.lines),
        format!("LEVEL   {}", snapshot.level),
        format!("SCORE   {}", snapshot.score),
        format!("TIME    {:02}:{:02}", elapsed / 60, elapsed % 60),
        format!("GRAVITY {} cells/s", snapshot.gravity_cps),
    ]
    .iter()
    .enumerate()
    {
        text(
            view,
            value,
            STATUS_X + 30.0,
            115.0 + index as f32 * 29.0,
            20.0,
            RED,
        );
    }
    text(view, "HOLD", HOLD_X + 30.0, 365.0, 28.0, RED);
    draw_piece(view, snapshot.hold, HOLD_X + 30.0, 400.0, 18.0);
    text(view, "NEXT", NEXT_X + 30.0, 82.0, 28.0, RED);
    for (index, kind) in snapshot.next.iter().take(5).enumerate() {
        draw_piece(
            view,
            Some(*kind),
            NEXT_X + 30.0,
            115.0 + index as f32 * 88.0,
            16.0,
        );
    }

    if let Some(notice) = snapshot.clear_notice {
        outline(view, NOTICE_X, 280.0, 350.0, 50.0, 2.0, RED);
        draw_clear_notice(view, notice, NOTICE_X + 15.0, 290.0);
    }
    if let Some(chain) = chain_number(snapshot.combo) {
        fragments(
            view,
            &[("CHAIN|", WHITE), (&chain.to_string(), RED)],
            WELL_CENTER_X,
            640.0,
            28.0,
            true,
        );
    }
    if show_seed {
        text(
            view,
            &format!("SEED {}", snapshot.seed),
            24.0,
            680.0,
            20.0,
            BLUE,
        );
    }

    text(view, "CONTROLS", 1010.0, 75.0, 28.0, RED);
    for (index, value) in if replay_playback {
        [
            "REPLAY",
            "P/ESC       PAUSE",
            "NO LIVE INPUT",
            "",
            "",
            "",
            "",
        ]
    } else {
        [
            "ARROWS/A D  MOVE",
            "UP/X, Z     ROTATE",
            "DOWN/S      SOFT DROP",
            "SPACE       HARD DROP",
            "C/H         HOLD",
            "P           PAUSE",
            "ESC         PAUSE",
        ]
    }
    .iter()
    .enumerate()
    {
        text(view, value, 1010.0, 112.0 + index as f32 * 23.0, 20.0, RED);
    }

    match snapshot.phase {
        Phase::Ready => centered_text(view, "GET READY", WELL_CENTER_X, 350.0, 54.0, WHITE),
        Phase::StartCountdown | Phase::ResumeCountdown => centered_text(
            view,
            &countdown_label(snapshot).to_string(),
            WELL_CENTER_X,
            350.0,
            28.0,
            WHITE,
        ),
        Phase::Paused => {
            centered_text(view, "PAUSED", WELL_CENTER_X, 300.0, 28.0, WHITE);
            for (index, option) in ["RESUME", "RESTART", "RETURN TO TITLE"].iter().enumerate() {
                centered_text(
                    view,
                    option,
                    WELL_CENTER_X,
                    390.0 + index as f32 * 30.0,
                    28.0,
                    if pause_index == index as u8 {
                        WHITE
                    } else {
                        RED
                    },
                );
            }
        }
        Phase::GameOver => centered_text(
            view,
            "GAME OVER — R TO RESTART",
            WELL_CENTER_X,
            350.0,
            28.0,
            WHITE,
        ),
        Phase::Complete => centered_text(
            view,
            "MARATHON COMPLETE — R TO RESTART",
            WELL_CENTER_X,
            350.0,
            28.0,
            WHITE,
        ),
        _ => {}
    }
    if !replay_playback && matches!(snapshot.phase, Phase::GameOver | Phase::Complete) {
        centered_text(
            view,
            "PRESS F5 TO SAVE REPLAY",
            WELL_CENTER_X,
            390.0,
            20.0,
            WHITE,
        );
    }
    if save.prompt {
        centered_text(view, "SAVE REPLAY AS", WELL_CENTER_X, 430.0, 24.0, WHITE);
        centered_text(
            view,
            if save.name.is_empty() {
                "marathon"
            } else {
                save.name
            },
            WELL_CENTER_X,
            465.0,
            24.0,
            WHITE,
        );
        centered_text(
            view,
            "ENTER TO SAVE • ESC TO CANCEL",
            WELL_CENTER_X,
            500.0,
            16.0,
            RED,
        );
    }
    if let Some(message) = save.message {
        centered_text(view, message, WELL_CENTER_X, 540.0, 16.0, WHITE);
    }
}

fn draw_active(
    view: View,
    snapshot: &GameSnapshot,
    board_x: f32,
    board_y: f32,
    cell: f32,
    ghost: bool,
) {
    let y = if ghost {
        snapshot.ghost_y
    } else {
        snapshot.active.y
    };
    for &(dx, dy) in shape_cells(snapshot.active.kind, snapshot.active.rotation) {
        let cell_y = y + dy;
        if !(HIDDEN_ROWS..HEIGHT).contains(&cell_y) {
            continue;
        }
        let x = snapshot.active.x + dx;
        let y = cell_y - HIDDEN_ROWS;
        if ghost {
            outline(
                view,
                board_x + x as f32 * cell,
                board_y + y as f32 * cell,
                cell - 2.0,
                cell - 2.0,
                5.0,
                GHOST_RED,
            );
        } else {
            rect(
                view,
                board_x + x as f32 * cell,
                board_y + y as f32 * cell,
                cell - 2.0,
                cell - 2.0,
                RED,
            );
        }
    }
}

fn draw_piece(view: View, kind: Option<PieceKind>, origin_x: f32, origin_y: f32, cell: f32) {
    let Some(kind) = kind else { return };
    for &(x, y) in shape_cells(kind, 0) {
        rect(
            view,
            origin_x + x as f32 * cell,
            origin_y + y as f32 * cell,
            cell - 2.0,
            cell - 2.0,
            RED,
        );
    }
}

fn draw_title(
    view: View,
    seed_control: &SeedControl,
    screen: TitleScreen,
    index: u8,
    replays: &[ReplayEntry],
) {
    fragments(
        view,
        &[("TETRA|", WHITE), ("D", RED)],
        640.0,
        210.0,
        54.0,
        true,
    );
    match screen {
        TitleScreen::Root => {
            for (i, value) in ["SOLO", "REPLAY", "CONFIG"].iter().enumerate() {
                centered_text(
                    view,
                    &format!("{}{}", if index == i as u8 { "> " } else { "" }, value),
                    640.0,
                    480.0 + i as f32 * 40.0,
                    28.0,
                    if index == i as u8 { WHITE } else { RED },
                );
            }
        }
        TitleScreen::Solo => {
            centered_text(view, "SOLO", 640.0, 440.0, 28.0, RED);
            centered_text(view, "> MARATHON", 640.0, 480.0, 28.0, WHITE);
        }
        TitleScreen::Replay => {
            centered_text(view, "REPLAY", 640.0, 440.0, 28.0, RED);
            if replays.is_empty() {
                centered_text(view, "NO REPLAYS FOUND", 640.0, 500.0, 24.0, WHITE);
            }
            for (row, entry) in replays
                .iter()
                .skip(index.saturating_sub(3) as usize)
                .take(7)
                .enumerate()
            {
                let selected = index as usize == row + index.saturating_sub(3) as usize;
                let detail = match &entry.replay {
                    Ok(replay) => format!(
                        "{}  SCORE {}  SEED {}  {}s",
                        entry.name,
                        replay.final_state.result.score,
                        replay.initial_seed,
                        replay.final_state.timeline_ticks / TICK_HZ as u64
                    ),
                    Err(error) => format!("{}  INVALID: {error}", entry.name),
                };
                text(
                    view,
                    &detail,
                    300.0,
                    480.0 + row as f32 * 28.0,
                    16.0,
                    if selected { WHITE } else { RED },
                );
            }
            centered_text(
                view,
                "ENTER TO PLAY • ESC TO RETURN",
                640.0,
                690.0,
                16.0,
                RED,
            );
        }
        TitleScreen::Config => {
            centered_text(view, "CONFIG — COMING SOON", 640.0, 500.0, 28.0, WHITE);
            centered_text(view, "ESC TO RETURN", 640.0, 550.0, 20.0, RED);
        }
    }
    if seed_control.prompt {
        centered_text(
            view,
            "ENTER SEED (DECIMAL OR 0xHEX)",
            640.0,
            300.0,
            28.0,
            WHITE,
        );
        centered_text(
            view,
            if seed_control.value.is_empty() {
                "_"
            } else {
                &seed_control.value
            },
            640.0,
            340.0,
            28.0,
            WHITE,
        );
        if seed_control.error {
            centered_text(
                view,
                "INVALID UNSIGNED 64-BIT SEED",
                640.0,
                375.0,
                20.0,
                RED,
            );
        }
    }
}

fn draw_clear_notice(view: View, notice: ClearNotice, x: f32, y: f32) {
    let prefix = match notice.spin {
        Some(Spin::Full) => "TSPIN ",
        Some(Spin::Mini) => "MINI TSPIN ",
        None => "",
    };
    let b2b = if notice.back_to_back { "B2B " } else { "" };
    if notice.clear == Clear::TetraD {
        fragments(
            view,
            &[
                ("", WHITE),
                (&format!("{b2b}{prefix}TETRA|"), WHITE),
                ("D", RED),
                (&format!("  +{}", notice.points), WHITE),
            ],
            x,
            y,
            28.0,
            false,
        );
    } else {
        text(
            view,
            &format!("{b2b}{prefix}{}  +{}", notice.clear.label(), notice.points),
            x,
            y,
            28.0,
            WHITE,
        );
    }
}

fn clearing_cell_is_white(rows: &[u8], clear_ticks: u32, board_row: u8, column: u8) -> bool {
    rows.contains(&board_row) && clear_ticks < (column as u32 + 1) * 25
}

fn level_flash_visible(remaining: u32) -> bool {
    let elapsed = 500_u32.saturating_sub(remaining);
    elapsed < 125 || (250..375).contains(&elapsed)
}

fn chain_number(combo: u32) -> Option<u32> {
    if combo >= 2 {
        Some(combo - 1)
    } else {
        None
    }
}

fn countdown_label(snapshot: &GameSnapshot) -> u32 {
    snapshot.timers.phase_remaining_ticks.div_ceil(1_000).max(1)
}

fn random_seed() -> u64 {
    // The seed is chosen outside the deterministic simulation.  Once passed to
    // Dynamo Core, every subsequent result is fully reproducible.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    nanos ^ nanos.rotate_left(17) ^ 0xA076_1D64_78BD_642F
}

fn drain_text_input() {
    while get_char_pressed().is_some() {}
}

fn save_name_character_allowed(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn new_recording(seed: u64) -> ReplayRecorder {
    ReplayRecorder::new(seed, GameConfig::default()).expect("default replay configuration is valid")
}

fn save_replay(recorder: &ReplayRecorder, name: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = format!("replays/{name}-{timestamp}.tdr.json");
    match recorder.replay().to_json_pretty().and_then(|json| {
        fs::create_dir_all("replays")
            .map_err(|error| tetradynamo_core::ReplayError::Invalid(error.to_string()))?;
        fs::write(&path, json)
            .map_err(|error| tetradynamo_core::ReplayError::Invalid(error.to_string()))
    }) {
        Ok(()) => format!("REPLAY SAVED: {path}"),
        Err(error) => format!("REPLAY SAVE FAILED: {error}"),
    }
}

fn discover_replays() -> Vec<ReplayEntry> {
    let mut entries = match fs::read_dir("replays") {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                (path
                    .extension()
                    .is_some_and(|extension| extension == "json"))
                .then_some(path)
            })
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                let replay = fs::read_to_string(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|json| Replay::from_json(&json).map_err(|error| error.to_string()));
                Some(ReplayEntry { name, replay })
            })
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    };
    entries.sort_by(|left, right| right.name.cmp(&left.name));
    entries
}

fn konami_code() -> Vec<KeyCode> {
    vec![
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::B,
        KeyCode::A,
    ]
}

fn konami_key() -> Option<KeyCode> {
    [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::A,
        KeyCode::B,
    ]
    .into_iter()
    .find(|key| is_key_pressed(*key))
}

fn parse_seed(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn seed_prompt_input(
    game: &mut ReplayRecorder,
    snapshot: &mut GameSnapshot,
    control: &mut SeedControl,
) {
    if is_key_pressed(KeyCode::Escape) {
        control.prompt = false;
        control.error = false;
        return;
    }
    if is_key_pressed(KeyCode::Backspace) {
        control.value.pop();
        control.error = false;
    }
    if is_key_pressed(KeyCode::Enter) {
        if let Some(seed) = parse_seed(&control.value) {
            control.locked = true;
            control.show = true;
            control.prompt = false;
            control.error = false;
            *snapshot = game.start_with_seed(seed).snapshot;
        } else {
            control.error = true;
        }
        return;
    }
    while let Some(character) = get_char_pressed() {
        if character.is_ascii_digit()
            || (character == 'x' || character == 'X')
                && (control.value == "0" || control.value.is_empty())
            || (character.is_ascii_hexdigit()
                && (control.value.starts_with("0x") || control.value.starts_with("0X")))
        {
            control.value.push(character);
            control.error = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_animation_matches_core_timing() {
        assert!(clearing_cell_is_white(&[21], 0, 21, 0));
        assert!(!clearing_cell_is_white(&[21], 25, 21, 0));
        assert!(clearing_cell_is_white(&[21], 249, 21, 9));
        assert!(!clearing_cell_is_white(&[21], 250, 21, 9));
    }

    #[test]
    fn chain_and_level_flash_match_other_frontends() {
        assert_eq!(chain_number(0), None);
        assert_eq!(chain_number(1), None);
        assert_eq!(chain_number(2), Some(1));
        assert!(level_flash_visible(500));
        assert!(!level_flash_visible(375));
        assert!(level_flash_visible(250));
    }

    #[test]
    fn seed_parser_accepts_decimal_and_hex() {
        assert_eq!(parse_seed("42"), Some(42));
        assert_eq!(parse_seed("0x2A"), Some(42));
        assert_eq!(parse_seed("18446744073709551616"), None);
    }

    #[test]
    fn save_names_use_portable_characters() {
        assert!(save_name_character_allowed('A'));
        assert!(save_name_character_allowed('_'));
        assert!(save_name_character_allowed('-'));
        assert!(!save_name_character_allowed('/'));
        assert!(!save_name_character_allowed(' '));
    }

    #[test]
    fn well_layout_is_horizontally_centered() {
        assert_eq!(BOARD_X, (LOGICAL_WIDTH - BOARD_WIDTH) / 2.0);
        assert_eq!(WELL_CENTER_X, LOGICAL_WIDTH / 2.0);
    }
}
