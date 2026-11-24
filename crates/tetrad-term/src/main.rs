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
    io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tetradynamo_core::{
    shape_cells, Board, Command, Game, Phase, PieceKind, HEIGHT, HIDDEN_ROWS, WIDTH,
};

const RED: Color = Color::Rgb(255, 0, 0);
const BLUE: Color = Color::Rgb(0, 0, 255);

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
    let mut game = Game::new(0x5445_5452_4144);
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;
    let mut pending_commands = Vec::new();
    let mut pause_index = 0_u8;
    let mut seed_control = SeedControl::default();
    let mut level_flash_ticks = 0_u32;
    loop {
        let now = Instant::now();
        acc += now - last;
        last = now;
        while acc >= Duration::from_millis(1) {
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
                if matches!(game.phase, Phase::Title | Phase::Paused)
                    && k.kind != KeyEventKind::Release
                    && register_konami(&mut seed_control.code, k.code)
                {
                    if game.phase == Phase::Title {
                        seed_control.prompt = true;
                        seed_control.value.clear();
                        seed_control.error = false;
                    } else {
                        seed_control.locked = !seed_control.locked;
                        seed_control.show = seed_control.locked;
                    }
                    continue;
                }
                if game.phase == Phase::Title && k.kind != KeyEventKind::Release {
                    if k.code == KeyCode::Enter {
                        let seed = if seed_control.locked {
                            game.snapshot().seed
                        } else {
                            random_seed()
                        };
                        game.start_with_seed(seed);
                    }
                    continue;
                }
                if game.phase == Phase::Paused && k.kind != KeyEventKind::Release {
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
                                game.restart_with_seed(seed);
                            }
                            _ => {
                                game.return_to_title();
                            }
                        },
                        KeyCode::Esc | KeyCode::Char('p') => pending_commands.push(Command::Resume),
                        _ => {}
                    }
                    continue;
                }
                if let Some(c) = command(k.code, k.kind) {
                    let c = if c == Command::Pause && game.phase == Phase::Paused {
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
                        game.restart_with_seed(seed);
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
            draw(
                f.area(),
                f,
                &game,
                level_flash_ticks,
                pause_index,
                &seed_control,
            )
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
    area: Rect,
    f: &mut ratatui::Frame,
    g: &Game,
    level_flash_ticks: u32,
    pause_index: u8,
    seed_control: &SeedControl,
) {
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );
    if g.phase == Phase::Title {
        title(area, f, seed_control);
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
        "LINES   {}\nLEVEL   {}\nSCORE   {}\nTIME    {:02}:{:02}\nGRAVITY {} cells/s",
        g.lines,
        g.level,
        g.score,
        g.snapshot().timers.active_gameplay_ticks / 60_000,
        (g.snapshot().timers.active_gameplay_ticks / 1_000) % 60,
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
            Paragraph::new(format!("SEED {}", g.snapshot().seed)).style(Style::default().fg(BLUE)),
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
    box_text(f,control,"CONTROLS","← → / A D  MOVE\n↑ X / Z     ROTATE\n↓ / S       SOFT DROP\nSPACE       HARD DROP\nC / H       HOLD\nP           PAUSE\nESC         QUIT".into());
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
    if matches!(
        g.phase,
        Phase::Paused
            | Phase::GameOver
            | Phase::Complete
            | Phase::Ready
            | Phase::StartCountdown
            | Phase::ResumeCountdown
    ) {
        let t = match g.phase {
            Phase::Paused => "PAUSED",
            Phase::GameOver => "GAME OVER — R TO RESTART",
            Phase::Complete => "MARATHON COMPLETE — R TO RESTART",
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

fn title(area: Rect, f: &mut ratatui::Frame, seed_control: &SeedControl) {
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
            "> MARATHON",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
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
            .draw(|frame| draw(frame.area(), frame, &game, 0, 0, &SeedControl::default()))
            .unwrap();
    }
}
