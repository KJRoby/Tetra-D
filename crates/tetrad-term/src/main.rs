use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::{
    fs, io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tetradynamo_core::{
    shape_cells, Board, Command, Game, GameConfig, Phase, PieceKind, Replay, ReplayPlayer,
    ReplayRecorder, HEIGHT, HIDDEN_ROWS, WIDTH,
};

const RED: Color = Color::Rgb(255, 0, 0);
const BLUE: Color = Color::Rgb(0, 0, 255);

#[derive(Clone, Copy, PartialEq)]
enum TitleMenu {
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
    paused: bool,
    ended: bool,
    error: Option<String>,
    selection: usize,
}

fn discover_replays() -> Vec<ReplayEntry> {
    let mut entries: Vec<_> = fs::read_dir("replays")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tdr.json"))
        .map(|e| ReplayEntry {
            name: e.file_name().to_string_lossy().into_owned(),
            replay: fs::read_to_string(e.path())
                .map_err(|e| e.to_string())
                .and_then(|s| Replay::from_json(&s).map_err(|e| e.to_string())),
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
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
fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let result = run(&mut Terminal::new(CrosstermBackend::new(out))?);
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}
fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut game =
        ReplayRecorder::new(0x5445_5452_4144, GameConfig::default()).expect("valid configuration");
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;
    let mut pending_commands = Vec::new();
    let mut pause_index = 0_u8;
    let mut title_index = 0_u8;
    let mut menu = TitleMenu::Root;
    let mut replay_index = 0_usize;
    let mut entries: Vec<ReplayEntry> = Vec::new();
    let mut playback: Option<Playback> = None;
    let mut save_name: Option<String> = None;
    let mut message = String::new();
    let mut seed_control = SeedControl::default();
    let mut level_flash_ticks = 0_u32;
    loop {
        let now = Instant::now();
        acc += now - last;
        last = now;
        while acc >= Duration::from_millis(1) {
            if let Some(active) = playback.as_mut() {
                if active.paused || active.ended {
                    acc = Duration::ZERO;
                    break;
                }
                match active.player.advance() {
                    Ok(Some(result)) => {
                        if result.events.iter().any(|event| {
                            matches!(event, tetradynamo_core::GameEvent::LevelChanged { .. })
                        }) {
                            level_flash_ticks = 500;
                        }
                    }
                    Ok(None) => {
                        active.ended = true;
                        active.error = active.player.finish().err().map(|e| e.to_string());
                    }
                    Err(error) => {
                        active.ended = true;
                        active.error = Some(error.to_string());
                    }
                }
                level_flash_ticks = level_flash_ticks.saturating_sub(1);
                acc -= Duration::from_millis(1);
                continue;
            }
            if game.game().phase == Phase::Paused && !pending_commands.contains(&Command::Resume) {
                acc = Duration::ZERO;
                break;
            }
            let batch = std::mem::take(&mut pending_commands);
            let result = game
                .step(&batch)
                .expect("terminal command batch is bounded");
            if result
                .events
                .iter()
                .any(|event| matches!(event, tetradynamo_core::GameEvent::LevelChanged { .. }))
            {
                level_flash_ticks = 500;
            }
            level_flash_ticks = level_flash_ticks.saturating_sub(1);
            acc -= Duration::from_millis(1);
        }
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Release && playback.is_some() {
                    continue;
                }
                if let Some(active) = playback.as_mut() {
                    if active.ended {
                        match k.code {
                            KeyCode::Esc => {
                                playback = None;
                                menu = TitleMenu::Root;
                                title_index = 0;
                                acc = Duration::ZERO;
                            }
                            KeyCode::Char('r') => match active.replay.player() {
                                Ok(player) => {
                                    active.player = player;
                                    active.ended = false;
                                    active.error = None;
                                    acc = Duration::ZERO;
                                }
                                Err(e) => active.error = Some(e.to_string()),
                            },
                            _ => {}
                        }
                    } else if matches!(k.code, KeyCode::Esc | KeyCode::Char('p')) {
                        active.paused = !active.paused;
                        active.selection = 0;
                        acc = Duration::ZERO;
                    } else if active.paused {
                        match k.code {
                            KeyCode::Up | KeyCode::Down => active.selection = 1 - active.selection,
                            KeyCode::Enter | KeyCode::Char(' ') if active.selection == 0 => {
                                active.paused = false;
                                acc = Duration::ZERO;
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                playback = None;
                                menu = TitleMenu::Root;
                                title_index = 0;
                                acc = Duration::ZERO;
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
                if let Some(name) = save_name.as_mut() {
                    if k.kind == KeyEventKind::Release {
                        continue;
                    }
                    match k.code {
                        KeyCode::Esc => save_name = None,
                        KeyCode::Backspace => {
                            name.pop();
                        }
                        KeyCode::Enter => {
                            let stem = if name.is_empty() {
                                game.game().config().mode.id()
                            } else {
                                name.as_str()
                            };
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let path = format!("replays/{stem}-{timestamp}.tdr.json");
                            message = match game
                                .replay()
                                .to_json_pretty()
                                .map_err(|e| e.to_string())
                                .and_then(|json| {
                                    fs::create_dir_all("replays")
                                        .and_then(|_| fs::write(&path, json))
                                        .map_err(|e| e.to_string())
                                }) {
                                Ok(()) => format!("SAVED {path}"),
                                Err(e) => format!("SAVE FAILED: {e}"),
                            };
                            save_name = None;
                        }
                        KeyCode::Char(c)
                            if name.len() < 48
                                && (c.is_ascii_alphanumeric() || c == '_' || c == '-') =>
                        {
                            name.push(c)
                        }
                        _ => {}
                    }
                    continue;
                }
                if seed_control.prompt && k.kind != KeyEventKind::Release {
                    match k.code {
                        KeyCode::Esc => {
                            seed_control.prompt = false;
                            seed_control.error = false;
                        }
                        KeyCode::Backspace => {
                            seed_control.value.pop();
                            seed_control.error = false;
                        }
                        KeyCode::Enter => match parse_seed(&seed_control.value) {
                            Some(seed) => {
                                seed_control.locked = true;
                                seed_control.show = true;
                                seed_control.prompt = false;
                                seed_control.error = false;
                                game = ReplayRecorder::new(seed, title_config(title_index))
                                    .expect("valid configuration");
                                game.start_with_seed(seed);
                            }
                            None => seed_control.error = true,
                        },
                        KeyCode::Char(character)
                            if seed_character_allowed(&seed_control.value, character) =>
                        {
                            seed_control.value.push(character);
                            seed_control.error = false;
                        }
                        _ => {}
                    }
                    continue;
                }
                if matches!(game.game().phase, Phase::Title | Phase::Paused)
                    && k.kind != KeyEventKind::Release
                    && register_konami(&mut seed_control.code, k.code)
                {
                    if game.game().phase == Phase::Title {
                        seed_control.prompt = true;
                        seed_control.value.clear();
                        seed_control.error = false;
                    } else {
                        seed_control.locked = !seed_control.locked;
                        seed_control.show = seed_control.locked;
                    }
                    continue;
                }
                if game.game().phase == Phase::Title && k.kind != KeyEventKind::Release {
                    if menu != TitleMenu::Solo {
                        match (menu, k.code) {
                            (TitleMenu::Root, KeyCode::Up) => title_index = (title_index + 2) % 3,
                            (TitleMenu::Root, KeyCode::Down) => title_index = (title_index + 1) % 3,
                            (TitleMenu::Root, KeyCode::Enter | KeyCode::Char(' ')) => {
                                menu = match title_index {
                                    0 => TitleMenu::Solo,
                                    1 => {
                                        entries = discover_replays();
                                        replay_index = 0;
                                        TitleMenu::Replay
                                    }
                                    _ => TitleMenu::Config,
                                };
                                title_index = 0;
                            }
                            (_, KeyCode::Esc) => {
                                menu = TitleMenu::Root;
                                title_index = 0;
                            }
                            (TitleMenu::Replay, KeyCode::Up) if !entries.is_empty() => {
                                replay_index = (replay_index + entries.len() - 1) % entries.len()
                            }
                            (TitleMenu::Replay, KeyCode::Down) if !entries.is_empty() => {
                                replay_index = (replay_index + 1) % entries.len()
                            }
                            (TitleMenu::Replay, KeyCode::Enter | KeyCode::Char(' ')) => {
                                if let Some(entry) = entries.get(replay_index) {
                                    match &entry.replay {
                                        Ok(replay) => match replay.player() {
                                            Ok(player) => {
                                                playback = Some(Playback {
                                                    replay: replay.clone(),
                                                    player,
                                                    paused: false,
                                                    ended: false,
                                                    error: None,
                                                    selection: 0,
                                                });
                                                acc = Duration::ZERO;
                                                pending_commands.clear();
                                            }
                                            Err(e) => message = e.to_string(),
                                        },
                                        Err(e) => message = e.clone(),
                                    }
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match k.code {
                        KeyCode::Esc => {
                            menu = TitleMenu::Root;
                            title_index = 0;
                        }
                        KeyCode::Up => title_index = (title_index + 1) % 2,
                        KeyCode::Down => title_index = (title_index + 1) % 2,
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            let seed = if seed_control.locked {
                                game.snapshot().seed
                            } else {
                                random_seed()
                            };
                            game = ReplayRecorder::new(seed, title_config(title_index))
                                .expect("valid configuration");
                            game.start_with_seed(seed);
                        }
                        _ => {}
                    }
                    continue;
                }
                if matches!(game.game().phase, Phase::GameOver | Phase::Complete)
                    && k.kind != KeyEventKind::Release
                {
                    match k.code {
                        KeyCode::F(5) => {
                            save_name = Some(String::new());
                            message.clear();
                        }
                        KeyCode::Esc => {
                            game.return_to_title();
                            menu = TitleMenu::Root;
                            title_index = 0;
                        }
                        KeyCode::Char('r') => {
                            let seed = if seed_control.locked {
                                game.snapshot().seed
                            } else {
                                random_seed()
                            };
                            game = ReplayRecorder::new(seed, game.game().config().clone())
                                .expect("valid configuration");
                            game.start_with_seed(seed);
                        }
                        _ => {}
                    }
                    continue;
                }
                if game.game().phase == Phase::Paused && k.kind != KeyEventKind::Release {
                    match k.code {
                        KeyCode::Up => pause_index = (pause_index + 2) % 3,
                        KeyCode::Down => pause_index = (pause_index + 1) % 3,
                        KeyCode::Enter | KeyCode::Char(' ') => match pause_index {
                            0 => pending_commands.push(Command::Resume),
                            1 => {
                                let seed = if seed_control.locked {
                                    game.snapshot().seed
                                } else {
                                    random_seed()
                                };
                                game = ReplayRecorder::new(seed, game.game().config().clone())
                                    .expect("valid configuration");
                                game.start_with_seed(seed);
                            }
                            _ => {
                                game.return_to_title();
                                menu = TitleMenu::Root;
                                title_index = 0;
                            }
                        },
                        KeyCode::Esc | KeyCode::Char('p') => pending_commands.push(Command::Resume),
                        _ => {}
                    }
                    continue;
                }
                if let Some(c) = command(k.code, k.kind) {
                    let c = if c == Command::Pause && game.game().phase == Phase::Paused {
                        Command::Resume
                    } else {
                        c
                    };
                    if c == Command::Restart {
                        let seed = if seed_control.locked {
                            game.snapshot().seed
                        } else {
                            random_seed()
                        };
                        game = ReplayRecorder::new(seed, game.game().config().clone())
                            .expect("valid configuration");
                        game.start_with_seed(seed);
                    } else {
                        pending_commands.push(c);
                    }
                    // Key-release events are not consistently available across
                    // Windows, macOS, and Linux terminals. Treat a press as a
                    // reliable tap; terminals that repeat a held key still give
                    // traditional repeat movement without a latched direction.
                    if k.kind != KeyEventKind::Release {
                        if let Some(release) = release_for(c) {
                            pending_commands.push(release);
                        }
                    }
                }
            }
        }
        terminal.draw(|f| {
            let area = f.area();
            if playback.is_none()
                && game.game().phase == Phase::Title
                && menu != TitleMenu::Solo
                && !seed_control.prompt
            {
                let lines: Vec<String> = match menu {
                    TitleMenu::Root => ["SOLO", "REPLAY", "CONFIG"]
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            format!("{} {s}", if i == title_index as usize { ">" } else { " " })
                        })
                        .collect(),
                    TitleMenu::Replay => {
                        if entries.is_empty() {
                            vec!["NO REPLAYS FOUND".into()]
                        } else {
                            entries
                                .iter()
                                .enumerate()
                                .skip(replay_index.saturating_sub(3))
                                .take(7)
                                .map(|(i, e)| {
                                    let detail = match &e.replay {
                                        Ok(r) => format!(
                                            "{}  0x{:X}  {}s  {}  {:?}",
                                            r.config.mode.id(),
                                            r.initial_seed,
                                            r.final_state.timeline_ticks / 1000,
                                            r.final_state.result.score,
                                            r.final_state.result.phase
                                        ),
                                        Err(err) => format!("INVALID: {err}"),
                                    };
                                    format!(
                                        "{} {} {detail}",
                                        if i == replay_index { ">" } else { " " },
                                        e.name
                                    )
                                })
                                .collect()
                        }
                    }
                    TitleMenu::Config => vec!["CONFIG — COMING SOON".into()],
                    TitleMenu::Solo => unreachable!(),
                };
                f.render_widget(
                    Paragraph::new(format!("TETRA|D\n\n{}\n\n{}", lines.join("\n"), message))
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(RED)),
                        ),
                    area,
                );
                return;
            }
            draw(
                f,
                playback
                    .as_ref()
                    .map_or_else(|| game.game(), |active| active.player.game()),
                level_flash_ticks,
                pause_index,
                &seed_control,
                title_index,
                playback.is_some(),
            );
            let overlay = if let Some(active) = playback.as_ref() {
                if active.ended {
                    Some(format!(
                        "{}\n{}",
                        if active.error.is_some() {
                            "REPLAY ERROR"
                        } else {
                            "REPLAY ENDED"
                        },
                        active
                            .error
                            .as_deref()
                            .unwrap_or("R REPLAY AGAIN  ESC TITLE")
                    ))
                } else if active.paused {
                    Some(format!(
                        "REPLAY PAUSED\n{} RESUME\n{} RETURN TO TITLE",
                        if active.selection == 0 { ">" } else { " " },
                        if active.selection == 1 { ">" } else { " " }
                    ))
                } else {
                    Some("REPLAY".into())
                }
            } else if let Some(name) = &save_name {
                Some(format!(
                    "SAVE REPLAY AS\n{}\nENTER SAVE  ESC CANCEL",
                    if name.is_empty() {
                        game.game().config().mode.id()
                    } else {
                        name
                    }
                ))
            } else if matches!(game.game().phase, Phase::GameOver | Phase::Complete) {
                Some(format!("R RESTART  ESC TITLE\nF5 SAVE REPLAY\n{message}"))
            } else {
                None
            };
            if let Some(overlay) = overlay {
                let rect = if playback.as_ref().is_some_and(|p| !p.paused && !p.ended) {
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: 20.min(area.width),
                        height: 1,
                    }
                } else {
                    Rect {
                        x: area.x + area.width.saturating_sub(35) / 2,
                        y: area.y + area.height / 2,
                        width: 35.min(area.width),
                        height: 4.min(area.height),
                    }
                };
                f.render_widget(
                    Paragraph::new(overlay)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::White).bg(Color::Black)),
                    rect,
                );
            }
        })?;
        std::thread::sleep(Duration::from_millis(8));
    }
}

fn release_for(command: Command) -> Option<Command> {
    Some(match command {
        Command::LeftPress => Command::LeftRelease,
        Command::RightPress => Command::RightRelease,
        Command::SoftDropPress => Command::SoftDropRelease,
        _ => return None,
    })
}
fn command(k: KeyCode, kind: KeyEventKind) -> Option<Command> {
    let press = kind != KeyEventKind::Release;
    Some(match (k, press) {
        (KeyCode::Left, true) | (KeyCode::Char('a'), true) => Command::LeftPress,
        (KeyCode::Left, false) | (KeyCode::Char('a'), false) => Command::LeftRelease,
        (KeyCode::Right, true) | (KeyCode::Char('d'), true) => Command::RightPress,
        (KeyCode::Right, false) | (KeyCode::Char('d'), false) => Command::RightRelease,
        (KeyCode::Down, true) | (KeyCode::Char('s'), true) => Command::SoftDropPress,
        (KeyCode::Down, false) | (KeyCode::Char('s'), false) => Command::SoftDropRelease,
        (KeyCode::Up, true) | (KeyCode::Char('x'), true) => Command::RotateCw,
        (KeyCode::Char('z'), true) => Command::RotateCcw,
        (KeyCode::Char(' '), true) => Command::HardDrop,
        (KeyCode::Char('c'), true) | (KeyCode::Char('h'), true) => Command::Hold,
        (KeyCode::Esc, true) | (KeyCode::Char('p'), true) => Command::Pause,
        (KeyCode::Enter, true) => Command::Start,
        (KeyCode::Char('r'), true) => Command::Restart,
        _ => return None,
    })
}
fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    nanos ^ nanos.rotate_left(17) ^ 0xA076_1D64_78BD_642F
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

fn seed_character_allowed(value: &str, character: char) -> bool {
    character.is_ascii_digit()
        || (matches!(character, 'x' | 'X') && (value == "0" || value.is_empty()))
        || (character.is_ascii_hexdigit() && (value.starts_with("0x") || value.starts_with("0X")))
}

fn register_konami(code: &mut Vec<KeyCode>, key: KeyCode) -> bool {
    const KONAMI: [KeyCode; 10] = [
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Char('b'),
        KeyCode::Char('a'),
    ];
    let key = match key {
        KeyCode::Char(character) => KeyCode::Char(character.to_ascii_lowercase()),
        key => key,
    };
    if !matches!(
        key,
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Char('a' | 'b')
    ) {
        return false;
    }
    code.push(key);
    if code.len() > KONAMI.len() {
        code.remove(0);
    }
    if code.as_slice() == KONAMI {
        code.clear();
        true
    } else {
        false
    }
}

fn draw(
    f: &mut ratatui::Frame,
    g: &Game,
    level_flash_ticks: u32,
    pause_index: u8,
    seed_control: &SeedControl,
    title_index: u8,
    replay: bool,
) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );
    if g.phase == Phase::Title {
        title(area, f, seed_control, title_index);
        return;
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Length(25),
            Constraint::Length(28),
        ])
        .horizontal_margin(2)
        .split(area);
    let left = cols[0];
    let mid = cols[1];
    let right = cols[2];
    let board_area = Rect {
        x: mid.x,
        y: mid.y,
        width: mid.width,
        height: 22.min(mid.height.saturating_sub(1)),
    };
    let stats = format!(
        "LINES   {}\nLEVEL   {}\nSCORE   {}\nTIME    {}\nGRAVITY {} cells/s",
        g.lines,
        g.level,
        g.score,
        format_mode_time(g.snapshot().timers.mode_elapsed_ticks),
        g.gravity_cps()
    );
    box_text(f, left, "STATUS", stats);
    let h = piece_art(g.hold);
    box_text(
        f,
        Rect {
            x: left.x,
            y: left.y + 10,
            width: left.width,
            height: 8,
        },
        "HOLD",
        h,
    );
    f.render_widget(
        Paragraph::new(if matches!(g.phase, Phase::Falling | Phase::Clearing) {
            board_art(&g.board, g)
        } else {
            vec![]
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(if level_flash_visible(level_flash_ticks) {
                        Color::White
                    } else {
                        RED
                    }),
                ),
        )
        .style(Style::default().fg(RED)),
        board_area,
    );
    if let Some(chain) = chain_label(g.combo) {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "CHAIN|",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    chain.to_string(),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
            ]))
            .alignment(Alignment::Center),
            Rect {
                x: board_area.x,
                y: board_area.bottom(),
                width: board_area.width,
                height: 1,
            },
        );
    }
    if seed_control.show {
        f.render_widget(
            Paragraph::new(format!("SEED 0x{:X}", g.snapshot().seed))
                .style(Style::default().fg(BLUE)),
            Rect {
                x: area.x + 1,
                y: area.bottom().saturating_sub(1),
                width: area.width / 2,
                height: 1,
            },
        );
    }
    let next = g
        .next()
        .take(5)
        .map(|p| format!("{}\n", piece_art(Some(p))))
        .collect::<String>();
    box_text(f, right, "NEXT", next);
    let control = Rect {
        x: right.x,
        y: area.bottom().saturating_sub(8),
        width: right.width,
        height: 8,
    };
    box_text(f,control,"CONTROLS","← → / A D  MOVE\n↑ X / Z     ROTATE\n↓ / S       SOFT DROP\nSPACE       HARD DROP\nC / H       HOLD\nP / ESC     PAUSE".into());
    if let Some(notice) = g.clear_notice {
        f.render_widget(
            Paragraph::new(clear_notice_line(notice))
                .alignment(Alignment::Center)
                .style(Style::default().add_modifier(Modifier::BOLD)),
            Rect {
                x: left.x,
                y: left.y + 7,
                width: left.width,
                height: 2,
            },
        );
    }
    if !(replay && matches!(g.phase, Phase::GameOver | Phase::Complete | Phase::Paused))
        && matches!(
            g.phase,
            Phase::Paused
                | Phase::GameOver
                | Phase::Complete
                | Phase::Ready
                | Phase::StartCountdown
                | Phase::ResumeCountdown
        )
    {
        let t = match g.phase {
            Phase::Paused => "PAUSED",
            Phase::GameOver => "GAME OVER — R TO RESTART",
            Phase::Complete => {
                if g.config().mode.id() == "sprint" {
                    "SPRINT COMPLETE — R TO RESTART"
                } else {
                    "MARATHON COMPLETE — R TO RESTART"
                }
            }
            Phase::Ready => "GET READY",
            Phase::StartCountdown | Phase::ResumeCountdown => {
                let remaining = g.snapshot().timers.phase_remaining_ticks;
                if remaining > 2_000 {
                    "3"
                } else if remaining > 1_000 {
                    "2"
                } else {
                    "1"
                }
            }
            _ => unreachable!(),
        };
        f.render_widget(
            Paragraph::new(t)
                .alignment(Alignment::Center)
                .style(Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Rect {
                x: mid.x,
                y: mid.y + 10,
                width: mid.width,
                height: 2,
            },
        );
        if g.phase == Phase::Paused {
            for (index, option) in ["RESUME", "RESTART", "RETURN TO TITLE"].iter().enumerate() {
                f.render_widget(
                    Paragraph::new(*option).alignment(Alignment::Center).style(
                        Style::default().fg(if pause_index == index as u8 {
                            Color::White
                        } else {
                            RED
                        }),
                    ),
                    Rect {
                        x: mid.x,
                        y: mid.y + 12 + index as u16,
                        width: mid.width,
                        height: 1,
                    },
                );
            }
        }
    }
}

fn chain_label(combo: u32) -> Option<u32> {
    if combo >= 2 {
        Some(combo - 1)
    } else {
        None
    }
}

fn level_flash_visible(remaining: u32) -> bool {
    let elapsed = 500_u32.saturating_sub(remaining);
    elapsed < 125 || (250..375).contains(&elapsed)
}

fn title(area: Rect, f: &mut ratatui::Frame, seed_control: &SeedControl, title_index: u8) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "TETRA|",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("D", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "A DETERMINISTIC FALLING-BLOCK GAME",
            Style::default().fg(RED),
        )),
        Line::from(""),
        Line::from(Span::styled("GAME MODE:", Style::default().fg(RED))),
        Line::from(Span::styled(
            if title_index == 0 {
                "> MARATHON"
            } else {
                "  MARATHON"
            },
            Style::default()
                .fg(if title_index == 0 { Color::White } else { RED })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if title_index == 1 {
                "> SPRINT"
            } else {
                "  SPRINT"
            },
            Style::default()
                .fg(if title_index == 1 { Color::White } else { RED })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "PRESS ENTER TO START",
            Style::default().fg(RED),
        )),
    ];
    if seed_control.prompt {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "ENTER SEED (DECIMAL OR 0xHEX)",
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(Span::styled(
            if seed_control.value.is_empty() {
                "_"
            } else {
                &seed_control.value
            },
            Style::default().fg(Color::White),
        )));
        if seed_control.error {
            lines.push(Line::from(Span::styled(
                "INVALID UNSIGNED 64-BIT SEED",
                Style::default().fg(RED),
            )));
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED)),
            )
            .style(Style::default().bg(Color::Black)),
        Rect {
            x: area.x + area.width / 4,
            y: area.y + area.height / 5,
            width: area.width / 2,
            height: area.height * 3 / 5,
        },
    );
}

fn title_config(index: u8) -> GameConfig {
    if index == 1 {
        GameConfig::guideline_sprint()
    } else {
        GameConfig::guideline_marathon()
    }
}

fn format_mode_time(ticks: u64) -> String {
    let hours = ticks / 3_600_000;
    let minutes = (ticks / 60_000) % 60;
    let seconds = (ticks / 1_000) % 60;
    let centiseconds = (ticks / 10) % 100;
    format!("{hours:02}:{minutes:02}:{seconds:02}:{centiseconds:02}")
}
fn box_text(f: &mut ratatui::Frame, r: Rect, title: &str, text: String) {
    f.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED)),
            )
            .style(Style::default().fg(RED)),
        r,
    )
}
fn board_art(b: &Board, g: &Game) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let animation = g.clearing_animation();
    for y in HIDDEN_ROWS..HEIGHT {
        let mut spans = Vec::new();
        for x in 0..WIDTH {
            let fade_cell = animation.is_some_and(|(rows, elapsed)| {
                rows.contains(&(y as usize)) && elapsed >= ((x as u32 + 1) * 25)
            });
            let clearing_cell = animation.is_some_and(|(rows, _)| rows.contains(&(y as usize)));
            let active = piece_cells(g.active.kind, g.active.rotation)
                .iter()
                .any(|&(dx, dy)| g.active.x + dx == x && g.active.y + dy == y);
            let ghost = piece_cells(g.active.kind, g.active.rotation)
                .iter()
                .any(|&(dx, dy)| g.active.x + dx == x && g.ghost_y() + dy == y);
            let (cell, color) = if clearing_cell && !fade_cell {
                ("██", Color::White)
            } else if fade_cell {
                ("··", RED)
            } else if active || b.cell(x, y).is_some() {
                ("██", RED)
            } else if ghost {
                ("░░", RED)
            } else {
                ("··", RED)
            };
            spans.push(Span::styled(cell, Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn clear_notice_line(notice: tetradynamo_core::ClearNotice) -> Line<'static> {
    let white = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let b2b = if notice.back_to_back { "B2B " } else { "" };
    let prefix = match notice.spin {
        Some(tetradynamo_core::Spin::Full) => format!("{b2b}TSPIN "),
        Some(tetradynamo_core::Spin::Mini) => format!("{b2b}MINI TSPIN "),
        None => b2b.to_owned(),
    };
    if notice.clear == tetradynamo_core::Clear::TetraD {
        return Line::from(vec![
            Span::styled(prefix + "TETRA|", white),
            Span::styled("D", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  +{}", notice.points), white),
        ]);
    }
    Line::from(Span::styled(
        format!("{}{}  +{}", prefix, notice.clear.label(), notice.points),
        white,
    ))
}
fn piece_art(p: Option<PieceKind>) -> String {
    let Some(p) = p else { return "(EMPTY)".into() };
    let mut s = String::new();
    for y in 0..4 {
        for x in 0..4 {
            s.push_str(if piece_cells(p, 0).contains(&(x, y)) {
                "██"
            } else {
                "  "
            });
        }
        s.push('\n');
    }
    s
}
fn piece_cells(k: PieceKind, r: u8) -> &'static [(i8, i8); 4] {
    shape_cells(k, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn gameplay_draws_after_start() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut game = Game::new(7);
        game.step(&[Command::Start]).unwrap();
        terminal
            .draw(|frame| draw(frame, &game, 0, 0, &SeedControl::default(), 0, false))
            .unwrap();
    }

    #[test]
    fn mode_time_has_hour_and_centisecond_precision() {
        assert_eq!(format_mode_time(0), "00:00:00:00");
        assert_eq!(format_mode_time(3_661_999), "01:01:01:99");
    }

    #[test]
    fn terminal_recording_and_fixture_play_back_through_core() {
        let mut recorder = ReplayRecorder::new(42, GameConfig::guideline_sprint()).unwrap();
        recorder.start_with_seed(42);
        for _ in 0..100 {
            recorder.step(&[]).unwrap();
        }
        let replay = Replay::from_json(&recorder.replay().to_json_pretty().unwrap()).unwrap();
        assert_eq!(replay.config.mode.id(), "sprint");
        assert_eq!(
            replay.verify().unwrap().final_state_hash,
            replay.final_state.state_hash
        );
        let fixture = include_str!("../../tetradynamo-core/tests/fixtures/ready-v1.tdr.json");
        assert!(Replay::from_json(fixture).unwrap().verify().is_ok());
    }
}
