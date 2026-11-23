use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dynamo_core::{
    shape_cells, Board, Command, Game, Phase, PieceKind, HEIGHT, HIDDEN_ROWS, WIDTH,
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
    time::{Duration, Instant},
};

const RED: Color = Color::Rgb(255, 0, 0);
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
    loop {
        let now = Instant::now();
        acc += now - last;
        last = now;
        while acc >= Duration::from_millis(1) {
            game.step(None);
            acc -= Duration::from_millis(1);
        }
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()? {
                if k.code == KeyCode::Esc {
                    return Ok(());
                }
                if let Some(c) = command(k.code, k.kind) {
                    game.step(Some(c));
                    // Key-release events are not consistently available across
                    // Windows, macOS, and Linux terminals. Treat a press as a
                    // reliable tap; terminals that repeat a held key still give
                    // traditional repeat movement without a latched direction.
                    if k.kind != KeyEventKind::Release {
                        if let Some(release) = release_for(c) {
                            game.step(Some(release));
                        }
                    }
                }
            }
        }
        terminal.draw(|f| draw(f.area(), f, &game))?;
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
        (KeyCode::Char('p'), true) => Command::Pause,
        (KeyCode::Enter, true) => Command::Start,
        (KeyCode::Char('r'), true) => Command::Restart,
        _ => return None,
    })
}
fn draw(area: Rect, f: &mut ratatui::Frame, g: &Game) {
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );
    if g.phase == Phase::Title {
        title(area, f);
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
    let stats = format!(
        "LINES   {}\nLEVEL   {}\nSCORE   {}\nTIME    {:02}:{:02}\nGRAVITY {} cells/s",
        g.lines,
        g.level,
        g.score,
        g.elapsed_ticks / 60_000,
        (g.elapsed_ticks / 1_000) % 60,
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
        Paragraph::new(board_art(&g.board, g))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED)),
            )
            .style(Style::default().fg(RED)),
        mid,
    );
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
    if let Some((c, p)) = g.clear_notice {
        let notice = format!("{}  +{}", c.label(), p);
        f.render_widget(
            Paragraph::new(notice)
                .alignment(Alignment::Center)
                .style(Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Rect {
                x: left.x,
                y: left.y + 7,
                width: left.width,
                height: 2,
            },
        );
    }
    if matches!(g.phase, Phase::Paused | Phase::GameOver | Phase::Complete) {
        let t = match g.phase {
            Phase::Paused => "PAUSED",
            Phase::GameOver => "GAME OVER — R TO RESTART",
            _ => "MARATHON COMPLETE — R TO RESTART",
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
    }
}
fn title(area: Rect, f: &mut ratatui::Frame) {
    let lines = vec![
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
fn board_art(b: &Board, g: &Game) -> String {
    let mut s = String::new();
    for y in HIDDEN_ROWS..HEIGHT {
        for x in 0..WIDTH {
            let active = piece_cells(g.active.kind, g.active.rotation)
                .iter()
                .any(|&(dx, dy)| g.active.x + dx == x && g.active.y + dy == y);
            let ghost = piece_cells(g.active.kind, g.active.rotation)
                .iter()
                .any(|&(dx, dy)| g.active.x + dx == x && g.ghost_y() + dy == y);
            s.push_str(if active || b.cell(x, y).is_some() {
                "██"
            } else if ghost {
                "░░"
            } else {
                "··"
            });
        }
        s.push('\n');
    }
    s
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
