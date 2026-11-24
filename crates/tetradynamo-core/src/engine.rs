//! Deterministic, frontend-independent falling-block simulation.
use crate::{Board, GameConfig, GravityCurve, RandomizerConfig, RotationSystem};
use std::collections::VecDeque;
pub const WIDTH: i8 = 10;
pub const HEIGHT: i8 = 22;
pub const HIDDEN_ROWS: i8 = 2;
pub const TICK_HZ: u32 = 1000;
pub const MAX_COMMANDS_PER_TICK: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}
impl PieceKind {
    pub const ALL: [Self; 7] = [
        Self::I,
        Self::J,
        Self::L,
        Self::O,
        Self::S,
        Self::T,
        Self::Z,
    ];
    fn id(self) -> u8 {
        match self {
            Self::I => 0,
            Self::J => 1,
            Self::L => 2,
            Self::O => 3,
            Self::S => 4,
            Self::T => 5,
            Self::Z => 6,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    LeftPress,
    LeftRelease,
    RightPress,
    RightRelease,
    SoftDropPress,
    SoftDropRelease,
    RotateCw,
    RotateCcw,
    HardDrop,
    Hold,
    Pause,
    Resume,
    Start,
    Restart,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    Falling,
    Clearing,
    Paused,
    GameOver,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clear {
    Monad,
    Dyad,
    Triad,
    TetraD,
}
impl Clear {
    pub fn label(self) -> &'static str {
        match self {
            Self::Monad => "MONAD",
            Self::Dyad => "DYAD",
            Self::Triad => "TRIAD",
            Self::TetraD => "TETRA|D",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spin {
    Full,
    Mini,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearNotice {
    pub clear: Clear,
    pub points: u32,
    pub spin: Option<Spin>,
    /// True when this qualifying clear received the back-to-back score bonus.
    pub back_to_back: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivePiece {
    pub kind: PieceKind,
    pub rotation: u8,
    pub x: i8,
    pub y: i8,
    last_rotation: bool,
    last_kick: (i8, i8),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    Started,
    Restarted {
        seed: u64,
    },
    Paused,
    Resumed,
    Spawned {
        kind: PieceKind,
    },
    Moved {
        dx: i8,
        dy: i8,
    },
    Rotated {
        rotation: u8,
        kick: (i8, i8),
    },
    Held {
        outgoing: PieceKind,
        incoming: PieceKind,
        hold: Option<PieceKind>,
    },
    HardDropped {
        distance: u8,
    },
    Locked {
        kind: PieceKind,
    },
    LinesCleared {
        rows: Vec<u8>,
        clear: Clear,
        spin: Option<Spin>,
        back_to_back: bool,
    },
    ScoreAwarded {
        points: u32,
        total: u64,
    },
    LevelChanged {
        from: u32,
        to: u32,
    },
    Combo {
        count: u32,
    },
    BackToBack {
        active: bool,
    },
    GameOver,
    Completed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepError {
    TooManyCommands { received: usize, maximum: usize },
}
#[derive(Clone, Debug)]
pub struct StepResult {
    pub events: Vec<GameEvent>,
    pub snapshot: GameSnapshot,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timers {
    pub simulation_ticks: u64,
    pub active_gameplay_ticks: u64,
    pub mode_elapsed_ticks: u64,
    pub gravity_ticks: u32,
    pub lock_ticks: u32,
    pub clear_ticks: u32,
    pub clear_notice_ticks: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSnapshot {
    pub board: [[Option<PieceKind>; WIDTH as usize]; HEIGHT as usize],
    pub active: ActivePiece,
    pub ghost_y: i8,
    pub hold: Option<PieceKind>,
    pub next: Vec<PieceKind>,
    pub phase: Phase,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    pub combo: u32,
    pub back_to_back: bool,
    pub gravity_cps: u32,
    pub clear_notice: Option<ClearNotice>,
    pub clearing_rows: Vec<u8>,
    pub timers: Timers,
}
#[derive(Clone, Debug)]
pub struct Game {
    config: GameConfig,
    pub board: Board,
    pub active: ActivePiece,
    pub hold: Option<PieceKind>,
    pub phase: Phase,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    pub combo: u32,
    pub back_to_back: bool,
    initial_seed: u64,
    rng: u64,
    queue: VecDeque<PieceKind>,
    sim: u64,
    active_time: u64,
    mode_time: u64,
    gravity: u32,
    lock: u32,
    clear: u32,
    notice_time: u32,
    pending: Vec<usize>,
    pub clear_notice: Option<ClearNotice>,
    hold_used: bool,
    left: bool,
    right: bool,
    spawn_buffer: Option<i8>,
    soft: bool,
    horizontal: u32,
    resets: u8,
}
impl Game {
    pub fn new(seed: u64) -> Self {
        Self::with_config(seed, GameConfig::default())
    }
    pub fn with_config(seed: u64, config: GameConfig) -> Self {
        let mut g = Self {
            config,
            board: Board::default(),
            active: piece(PieceKind::T),
            hold: None,
            phase: Phase::Title,
            score: 0,
            lines: 0,
            level: 1,
            combo: 0,
            back_to_back: false,
            initial_seed: seed,
            rng: seed_rng(seed),
            queue: VecDeque::new(),
            sim: 0,
            active_time: 0,
            mode_time: 0,
            gravity: 0,
            lock: 0,
            clear: 0,
            notice_time: 0,
            pending: vec![],
            clear_notice: None,
            hold_used: false,
            left: false,
            right: false,
            spawn_buffer: None,
            soft: false,
            horizontal: 0,
            resets: 0,
        };
        g.fill();
        g.active = piece(g.take());
        g
    }
    pub fn config(&self) -> &GameConfig {
        &self.config
    }
    pub fn initial_seed(&self) -> u64 {
        self.initial_seed
    }
    pub fn next(&self) -> impl Iterator<Item = PieceKind> + '_ {
        self.queue.iter().copied()
    }
    pub fn gravity_cps(&self) -> u32 {
        (1000 / gravity_ms(self.config.rules.gravity, self.level)).max(1)
    }
    pub fn ghost_y(&self) -> i8 {
        let mut d = 0;
        while !self.board.hit(&self.active, 0, d + 1, self.active.rotation) {
            d += 1
        }
        self.active.y + d
    }
    pub fn clearing_animation(&self) -> Option<(&[usize], u32)> {
        (self.phase == Phase::Clearing).then_some((&self.pending, self.clear))
    }
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            board: *self.board.cells(),
            active: self.active,
            ghost_y: self.ghost_y(),
            hold: self.hold,
            next: self.next().collect(),
            phase: self.phase,
            score: self.score,
            lines: self.lines,
            level: self.level,
            combo: self.combo,
            back_to_back: self.back_to_back,
            gravity_cps: self.gravity_cps(),
            clear_notice: self.clear_notice,
            clearing_rows: self.pending.iter().map(|&r| r as u8).collect(),
            timers: Timers {
                simulation_ticks: self.sim,
                active_gameplay_ticks: self.active_time,
                mode_elapsed_ticks: self.mode_time,
                gravity_ticks: self.gravity,
                lock_ticks: self.lock,
                clear_ticks: self.clear,
                clear_notice_ticks: self.notice_time,
            },
        }
    }
    pub fn state_hash(&self) -> u64 {
        let s = self.snapshot();
        let mut h = 0xcbf29ce484222325u64;
        let mut put = |x: u64| {
            h ^= x;
            h = h.wrapping_mul(0x100000001b3)
        };
        put(s.phase as u64);
        put(s.score);
        put(s.lines as u64);
        put(s.level as u64);
        put(s.combo as u64);
        put(s.back_to_back as u64);
        put(self.initial_seed);
        put(self.rng);
        put(self.active.last_rotation as u64);
        put(self.active.last_kick.0 as u8 as u64);
        put(self.active.last_kick.1 as u8 as u64);
        put(self.hold.map_or(255, PieceKind::id) as u64);
        put(self.hold_used as u64);
        put(self.left as u64);
        put(self.right as u64);
        put(self
            .spawn_buffer
            .map_or(0, |direction| (direction + 2) as u64));
        put(self.soft as u64);
        put(self.sim);
        put(self.active_time);
        put(self.mode_time);
        put(self.gravity as u64);
        put(self.lock as u64);
        put(self.clear as u64);
        put(self.notice_time as u64);
        put(self.horizontal as u64);
        put(self.resets as u64);
        put(self.pending.len() as u64);
        for row in &self.pending {
            put(*row as u64);
        }
        match self.clear_notice {
            Some(notice) => {
                put(1);
                put(notice.clear as u64);
                put(notice.points as u64);
                put(notice.spin.map_or(255, |spin| spin as u8) as u64);
                put(notice.back_to_back as u64);
            }
            None => put(0),
        }
        let rules = self.config.rules;
        put(rules.rotation as u64);
        put(rules.randomizer as u64);
        put(rules.gravity as u64);
        put(rules.timing.lock_delay_ticks as u64);
        put(rules.timing.clear_delay_ticks as u64);
        put(rules.timing.clear_notice_ticks as u64);
        put(rules.timing.max_lock_resets as u64);
        put(rules.input.das_ticks as u64);
        put(rules.input.arr_ticks as u64);
        put(rules.input.soft_drop_divisor as u64);
        put(rules.input.minimum_soft_drop_ticks as u64);
        put(rules.hold.enabled as u64);
        put(rules.hold.once_per_piece as u64);
        put(rules.spins.detect_t_spins as u64);
        put(rules.scoring.back_to_back_multiplier_numerator as u64);
        put(rules.scoring.back_to_back_multiplier_denominator as u64);
        let mode = self.config.mode.marathon_config();
        put(mode.target_lines as u64);
        put(mode.lines_per_level as u64);
        put(mode.maximum_level as u64);
        put(s.active.kind.id() as u64);
        put(s.active.rotation as u64);
        put(s.active.x as u64);
        put(s.active.y as u64);
        for r in s.board {
            for c in r {
                put(c.map_or(255, |p| p.id()) as u64)
            }
        }
        for p in s.next {
            put(p.id() as u64)
        }
        h
    }
    pub fn restart(&mut self) -> StepResult {
        let seed = self.initial_seed;
        *self = Self::with_config(seed, self.config.clone());
        StepResult {
            events: vec![
                GameEvent::Restarted { seed },
                GameEvent::Spawned {
                    kind: self.active.kind,
                },
            ],
            snapshot: self.snapshot(),
        }
    }
    pub fn step(&mut self, commands: &[Command]) -> Result<StepResult, StepError> {
        if commands.len() > MAX_COMMANDS_PER_TICK {
            return Err(StepError::TooManyCommands {
                received: commands.len(),
                maximum: MAX_COMMANDS_PER_TICK,
            });
        }
        let mut e = Vec::with_capacity(commands.len() + 8);
        for &c in commands {
            self.command(c, &mut e)
        }
        self.sim += 1;
        if self.notice_time > 0 {
            self.notice_time -= 1;
            if self.notice_time == 0 {
                self.clear_notice = None
            }
        }
        match self.phase {
            Phase::Falling => self.falling(&mut e),
            Phase::Clearing => self.clearing(&mut e),
            _ => {}
        }
        Ok(StepResult {
            events: e,
            snapshot: self.snapshot(),
        })
    }
    fn command(&mut self, c: Command, e: &mut Vec<GameEvent>) {
        match c {
            Command::Start if self.phase == Phase::Title => {
                self.phase = Phase::Falling;
                e.push(GameEvent::Started);
                e.push(GameEvent::Spawned {
                    kind: self.active.kind,
                })
            }
            Command::Restart if matches!(self.phase, Phase::GameOver | Phase::Complete) => {
                e.extend(self.restart().events)
            }
            Command::Pause if self.phase == Phase::Falling => {
                self.phase = Phase::Paused;
                self.clear_input();
                e.push(GameEvent::Paused)
            }
            Command::Resume if self.phase == Phase::Paused => {
                self.phase = Phase::Falling;
                e.push(GameEvent::Resumed)
            }
            Command::LeftRelease => {
                self.left = false;
                if self.spawn_buffer == Some(-1) {
                    self.spawn_buffer = None;
                }
            }
            Command::RightRelease => {
                self.right = false;
                if self.spawn_buffer == Some(1) {
                    self.spawn_buffer = None;
                }
            }
            Command::SoftDropRelease => {
                if self.soft {
                    self.soft = false;
                    self.gravity = 0;
                }
            }
            _ if !matches!(self.phase, Phase::Falling | Phase::Clearing) => {}
            Command::LeftPress => {
                self.left = true;
                self.horizontal = 0;
                if self.phase == Phase::Clearing {
                    self.spawn_buffer = Some(-1);
                } else {
                    self.move_x(-1, e)
                }
            }
            Command::RightPress => {
                self.right = true;
                self.horizontal = 0;
                if self.phase == Phase::Clearing {
                    self.spawn_buffer = Some(1);
                } else {
                    self.move_x(1, e)
                }
            }
            Command::SoftDropPress => {
                if !self.soft {
                    self.soft = true;
                    // Soft drop has a much shorter gravity interval. Reset the
                    // normal-gravity progress so it cannot be interpreted as
                    // many already-overdue soft-drop intervals.
                    self.gravity = 0;
                    self.down(true, e)
                }
            }
            Command::RotateCw => self.rotate(1, e),
            Command::RotateCcw => self.rotate(3, e),
            Command::HardDrop => self.hard_drop(e),
            Command::Hold => self.hold_piece(e),
            _ => {}
        }
    }
    fn falling(&mut self, e: &mut Vec<GameEvent>) {
        self.active_time += 1;
        self.mode_time += 1;
        self.repeat(e);
        let interval = if self.soft {
            (gravity_ms(self.config.rules.gravity, self.level)
                / self.config.rules.input.soft_drop_divisor)
                .max(self.config.rules.input.minimum_soft_drop_ticks)
        } else {
            gravity_ms(self.config.rules.gravity, self.level)
        };
        self.gravity += 1;
        if self.gravity >= interval {
            self.gravity -= interval;
            self.down(false, e)
        }
        if self.board.hit(&self.active, 0, 1, self.active.rotation) {
            self.lock += 1;
            if self.lock >= self.config.rules.timing.lock_delay_ticks {
                self.lock_piece(e)
            }
        } else {
            self.lock = 0
        }
    }
    fn clearing(&mut self, e: &mut Vec<GameEvent>) {
        self.mode_time += 1;
        self.clear += 1;
        if self.clear >= self.config.rules.timing.clear_delay_ticks {
            let n = self.pending.len() as u32;
            self.board.remove(&self.pending);
            self.pending.clear();
            self.lines += n;
            let old = self.level;
            let mode = self.config.mode.marathon_config();
            self.level = (1 + self.lines / mode.lines_per_level).min(mode.maximum_level);
            if old != self.level {
                e.push(GameEvent::LevelChanged {
                    from: old,
                    to: self.level,
                })
            }
            let next = self.take();
            self.activate(next, e)
        }
    }
    fn repeat(&mut self, e: &mut Vec<GameEvent>) {
        let d = if self.left {
            Some(-1)
        } else if self.right {
            Some(1)
        } else {
            None
        };
        let Some(d) = d else { return };
        self.horizontal += 1;
        let input = self.config.rules.input;
        if self.horizontal == input.das_ticks
            || (self.horizontal > input.das_ticks
                && (self.horizontal - input.das_ticks).is_multiple_of(input.arr_ticks))
        {
            self.move_x(d, e)
        }
    }
    fn move_x(&mut self, dx: i8, e: &mut Vec<GameEvent>) {
        if !self.board.hit(&self.active, dx, 0, self.active.rotation) {
            self.active.x += dx;
            self.active.last_rotation = false;
            self.reset_lock();
            e.push(GameEvent::Moved { dx, dy: 0 })
        }
    }
    fn down(&mut self, points: bool, e: &mut Vec<GameEvent>) {
        if !self.board.hit(&self.active, 0, 1, self.active.rotation) {
            self.active.y += 1;
            self.lock = 0;
            e.push(GameEvent::Moved { dx: 0, dy: 1 });
            if points {
                self.award(1, e)
            }
        }
    }
    fn rotate(&mut self, dir: u8, e: &mut Vec<GameEvent>) {
        let next = (self.active.rotation + dir) % 4;
        let kicks = match self.config.rules.rotation {
            RotationSystem::GuidelineSrs => kicks(self.active.kind, self.active.rotation, next),
        };
        for (dx, dy) in kicks {
            if !self.board.hit(&self.active, *dx, *dy, next) {
                self.active.x += dx;
                self.active.y += dy;
                self.active.rotation = next;
                self.active.last_rotation = true;
                self.active.last_kick = (*dx, *dy);
                self.reset_lock();
                e.push(GameEvent::Rotated {
                    rotation: next,
                    kick: (*dx, *dy),
                });
                break;
            }
        }
    }
    fn hard_drop(&mut self, e: &mut Vec<GameEvent>) {
        let y = self.ghost_y();
        let d = (y - self.active.y) as u8;
        self.active.y = y;
        e.push(GameEvent::HardDropped { distance: d });
        self.award(d as u32 * 2, e);
        self.lock_piece(e)
    }
    fn hold_piece(&mut self, e: &mut Vec<GameEvent>) {
        if !self.config.rules.hold.enabled
            || (self.config.rules.hold.once_per_piece && self.hold_used)
        {
            return;
        }
        let outgoing = self.active.kind;
        let incoming = match self.hold.replace(outgoing) {
            Some(x) => x,
            None => self.take(),
        };
        self.activate_raw(incoming);
        self.hold_used = true;
        e.push(GameEvent::Held {
            outgoing,
            incoming,
            hold: self.hold,
        });
        if self.phase == Phase::GameOver {
            e.push(GameEvent::GameOver)
        }
    }
    fn lock_piece(&mut self, e: &mut Vec<GameEvent>) {
        let spin = self
            .config
            .rules
            .spins
            .detect_t_spins
            .then(|| self.t_spin())
            .flatten();
        self.board.lock(&self.active);
        e.push(GameEvent::Locked {
            kind: self.active.kind,
        });
        self.lock = 0;
        self.resets = 0;
        let rows = self.board.full();
        if rows.is_empty() {
            self.combo = 0;
            if spin.is_some() {
                self.award(
                    score(ScoreInput {
                        lines: 0,
                        level: self.level,
                        t_spin: true,
                        mini: spin == Some(Spin::Mini),
                        combo: 0,
                        back_to_back: self.back_to_back,
                        perfect_clear: false,
                        scoring: self.config.rules.scoring,
                    }),
                    e,
                )
            }
            let next = self.take();
            self.activate(next, e);
            return;
        }
        let n = rows.len() as u32;
        let clear = match n {
            1 => Clear::Monad,
            2 => Clear::Dyad,
            3 => Clear::Triad,
            _ => Clear::TetraD,
        };
        self.combo += 1;
        let back_to_back = self.back_to_back && qualifies_back_to_back(spin, n);
        let points = score(ScoreInput {
            lines: n,
            level: self.level,
            t_spin: spin.is_some(),
            mini: spin == Some(Spin::Mini),
            combo: self.combo,
            back_to_back: self.back_to_back,
            perfect_clear: self.board.empty_after(&rows),
            scoring: self.config.rules.scoring,
        });
        self.award(points, e);
        e.push(GameEvent::LinesCleared {
            rows: rows.iter().map(|&r| r as u8).collect(),
            clear,
            spin,
            back_to_back,
        });
        e.push(GameEvent::Combo { count: self.combo });
        let b = qualifies_back_to_back(spin, n);
        if b != self.back_to_back {
            e.push(GameEvent::BackToBack { active: b })
        }
        self.back_to_back = b;
        self.clear_notice = Some(ClearNotice {
            clear,
            points,
            spin,
            back_to_back,
        });
        self.notice_time = self.config.rules.timing.clear_notice_ticks;
        self.pending = rows;
        self.clear = 0;
        self.phase = Phase::Clearing
    }
    fn activate_raw(&mut self, k: PieceKind) {
        self.active = piece(k);
        self.gravity = 0;
        self.lock = 0;
        self.resets = 0;
        self.horizontal = 0;
        self.hold_used = false;
        self.phase = if self.board.hit(&self.active, 0, 0, 0) {
            Phase::GameOver
        } else {
            Phase::Falling
        }
    }
    fn activate(&mut self, k: PieceKind, e: &mut Vec<GameEvent>) {
        if self.lines >= self.config.mode.marathon_config().target_lines {
            self.phase = Phase::Complete;
            e.push(GameEvent::Completed);
            return;
        }
        self.activate_raw(k);
        if self.phase == Phase::GameOver {
            e.push(GameEvent::GameOver)
        } else {
            let buffered_move = self
                .spawn_buffer
                .take()
                .filter(|&direction| match direction {
                    -1 => self.left,
                    1 => self.right,
                    _ => false,
                });
            e.push(GameEvent::Spawned { kind: k });
            if let Some(direction) = buffered_move {
                self.move_x(direction, e);
            }
        }
    }
    fn reset_lock(&mut self) {
        if self.board.hit(&self.active, 0, 1, self.active.rotation)
            && self.resets < self.config.rules.timing.max_lock_resets
        {
            self.lock = 0;
            self.resets += 1
        }
    }
    fn award(&mut self, p: u32, e: &mut Vec<GameEvent>) {
        if p > 0 {
            self.score += p as u64;
            e.push(GameEvent::ScoreAwarded {
                points: p,
                total: self.score,
            })
        }
    }
    fn t_spin(&self) -> Option<Spin> {
        if self.active.kind != PieceKind::T || !self.active.last_rotation {
            return None;
        }
        let (x, y) = (self.active.x + 1, self.active.y + 1);
        let c = [
            (x - 1, y - 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x + 1, y + 1),
        ];
        if c.iter()
            .filter(|&&(x, y)| self.board.occupied(x, y))
            .count()
            < 3
        {
            return None;
        }
        let f = match self.active.rotation {
            0 => [c[0], c[1]],
            1 => [c[1], c[3]],
            2 => [c[2], c[3]],
            _ => [c[0], c[2]],
        };
        if self.active.last_kick.1.abs() >= 2 || f.iter().all(|&(x, y)| self.board.occupied(x, y)) {
            Some(Spin::Full)
        } else {
            Some(Spin::Mini)
        }
    }
    fn clear_input(&mut self) {
        self.left = false;
        self.right = false;
        self.spawn_buffer = None;
        self.soft = false;
        self.horizontal = 0
    }
    fn random(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
    fn fill(&mut self) {
        match self.config.rules.randomizer {
            RandomizerConfig::SevenBag => {
                while self.queue.len() < 7 {
                    let mut b = PieceKind::ALL;
                    for i in (1..7).rev() {
                        let j = (self.random() % (i as u64 + 1)) as usize;
                        b.swap(i, j)
                    }
                    self.queue.extend(b)
                }
            }
        }
    }
    fn take(&mut self) -> PieceKind {
        self.fill();
        self.queue.pop_front().unwrap()
    }
}
fn seed_rng(s: u64) -> u64 {
    let mut z = s.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    let z = z ^ (z >> 31);
    if z == 0 {
        1
    } else {
        z
    }
}
struct ScoreInput {
    lines: u32,
    level: u32,
    t_spin: bool,
    mini: bool,
    combo: u32,
    back_to_back: bool,
    perfect_clear: bool,
    scoring: crate::ScoringConfig,
}

fn score(input: ScoreInput) -> u32 {
    let mut x = if input.t_spin {
        [400, 800, 1200, 1600][input.lines.min(3) as usize]
    } else {
        [0, 100, 300, 500, 800][input.lines as usize]
    };
    if input.t_spin && input.mini {
        x = 100 + [0, 100, 300, 0][input.lines.min(3) as usize]
    }
    let mut x = x * input.level;
    if input.back_to_back && (input.t_spin || input.lines == 4) {
        x = x * input.scoring.back_to_back_multiplier_numerator
            / input.scoring.back_to_back_multiplier_denominator
    }
    if input.combo >= 2 {
        x += 50 * (input.combo - 1) * input.level
    }
    if input.perfect_clear {
        x += [0, 800, 1000, 1800, 2000][input.lines as usize] * input.level
    }
    x
}
fn qualifies_back_to_back(spin: Option<Spin>, lines: u32) -> bool {
    spin.is_some() || lines == 4
}
fn gravity_ms(curve: GravityCurve, l: u32) -> u32 {
    match curve {
        GravityCurve::Guideline => [
            1000, 793, 618, 473, 355, 262, 190, 135, 94, 64, 43, 28, 18, 11, 7,
        ][l.saturating_sub(1).min(14) as usize],
    }
}
fn piece(k: PieceKind) -> ActivePiece {
    ActivePiece {
        kind: k,
        rotation: 0,
        x: 3,
        y: 0,
        last_rotation: false,
        last_kick: (0, 0),
    }
}
pub fn shape_cells(k: PieceKind, r: u8) -> &'static [(i8, i8); 4] {
    const I: [[(i8, i8); 4]; 4] = [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ];
    const O: [[(i8, i8); 4]; 4] = [[(1, 0), (2, 0), (1, 1), (2, 1)]; 4];
    const J: [[(i8, i8); 4]; 4] = [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ];
    const L: [[(i8, i8); 4]; 4] = [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ];
    const S: [[(i8, i8); 4]; 4] = [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ];
    const Z: [[(i8, i8); 4]; 4] = [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ];
    const T: [[(i8, i8); 4]; 4] = [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ];
    match k {
        PieceKind::I => &I[r as usize],
        PieceKind::O => &O[r as usize],
        PieceKind::J => &J[r as usize],
        PieceKind::L => &L[r as usize],
        PieceKind::S => &S[r as usize],
        PieceKind::Z => &Z[r as usize],
        PieceKind::T => &T[r as usize],
    }
}
fn kicks(k: PieceKind, f: u8, t: u8) -> &'static [(i8, i8)] {
    const A: &[(i8, i8)] = &[(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)];
    const B: &[(i8, i8)] = &[(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)];
    const C: &[(i8, i8)] = &[(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)];
    const D: &[(i8, i8)] = &[(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)];
    const IA: &[(i8, i8)] = &[(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)];
    const IB: &[(i8, i8)] = &[(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)];
    const IC: &[(i8, i8)] = &[(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)];
    const ID: &[(i8, i8)] = &[(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)];
    if k == PieceKind::O {
        return &[(0, 0)];
    }
    match (k == PieceKind::I, f, t) {
        (false, 0, 1) | (false, 2, 1) => A,
        (false, 1, 0) | (false, 1, 2) => B,
        (false, 2, 3) | (false, 0, 3) => C,
        (false, 3, 2) | (false, 3, 0) => D,
        (true, 0, 1) => IA,
        (true, 1, 0) => IB,
        (true, 1, 2) => IC,
        (true, 2, 1) => ID,
        (true, 2, 3) => IB,
        (true, 3, 2) => IA,
        (true, 3, 0) => ID,
        (true, 0, 3) => IC,
        _ => &[(0, 0)],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_seed_bag() {
        let a = Game::new(0);
        let v: Vec<_> = a
            .next()
            .take(6)
            .chain(std::iter::once(a.active.kind))
            .collect();
        assert!(PieceKind::ALL.iter().all(|x| v.contains(x)))
    }
    #[test]
    fn one_tick_for_batch() {
        let mut g = Game::new(1);
        g.step(&[Command::Start]).unwrap();
        let n = g.snapshot().timers.simulation_ticks;
        g.step(&[Command::RotateCw, Command::HardDrop]).unwrap();
        assert_eq!(g.snapshot().timers.simulation_ticks, n + 1)
    }
    #[test]
    fn restart_is_seeded() {
        let mut g = Game::new(0);
        g.phase = Phase::GameOver;
        g.restart();
        assert_eq!(g.state_hash(), Game::new(0).state_hash())
    }
    #[test]
    fn oversized_batch_is_rejected_without_advancing() {
        let mut game = Game::new(7);
        let commands = vec![Command::RotateCw; MAX_COMMANDS_PER_TICK + 1];
        assert!(matches!(
            game.step(&commands),
            Err(StepError::TooManyCommands { .. })
        ));
        assert_eq!(game.snapshot().timers.simulation_ticks, 0);
    }
    #[test]
    fn pause_clears_latched_input() {
        let mut game = Game::new(7);
        game.step(&[
            Command::Start,
            Command::LeftPress,
            Command::SoftDropPress,
            Command::Pause,
        ])
        .unwrap();
        assert!(!game.left && !game.right && !game.soft);
        game.step(&[Command::Resume]).unwrap();
        assert!(!game.left && !game.right && !game.soft);
    }
    #[test]
    fn only_resume_can_unpause() {
        let mut game = Game::new(7);
        game.step(&[Command::Start, Command::Pause]).unwrap();
        game.step(&[Command::RotateCw, Command::HardDrop, Command::Pause])
            .unwrap();
        assert_eq!(game.phase, Phase::Paused);
        game.step(&[Command::Resume]).unwrap();
        assert_eq!(game.phase, Phase::Falling);
    }
    #[test]
    fn hold_is_limited_until_lock() {
        let mut game = Game::new(7);
        game.step(&[Command::Start, Command::Hold, Command::Hold])
            .unwrap();
        assert!(game.hold_used);
    }
    #[test]
    fn repeated_soft_drop_presses_only_apply_the_initial_drop() {
        let mut game = Game::new(7);
        game.step(&[Command::Start]).unwrap();
        let initial_y = game.active.y;
        game.step(&[Command::SoftDropPress, Command::SoftDropPress])
            .unwrap();
        assert_eq!(game.active.y, initial_y + 1);
    }
    #[test]
    fn soft_drop_does_not_inherit_normal_gravity_progress() {
        let mut game = Game::new(7);
        game.step(&[Command::Start]).unwrap();
        game.gravity = gravity_ms(game.config.rules.gravity, game.level) - 1;
        let initial_y = game.active.y;

        game.step(&[Command::SoftDropPress]).unwrap();
        assert_eq!(game.active.y, initial_y + 1);

        game.step(&[]).unwrap();
        assert_eq!(game.active.y, initial_y + 1);
    }
    #[test]
    fn releasing_soft_drop_resets_the_normal_gravity_phase() {
        let mut game = Game::new(7);
        game.step(&[Command::Start, Command::SoftDropPress])
            .unwrap();
        game.gravity = 49;
        game.step(&[Command::SoftDropRelease]).unwrap();
        assert_eq!(game.gravity, 1);
    }
    #[test]
    fn back_to_back_qualifiers_match_tspins_and_tetrad() {
        assert!(qualifies_back_to_back(Some(Spin::Full), 1));
        assert!(qualifies_back_to_back(Some(Spin::Mini), 1));
        assert!(qualifies_back_to_back(None, 4));
        assert!(!qualifies_back_to_back(None, 3));
    }
    #[test]
    fn default_configuration_matches_legacy_constructor() {
        let trace = [
            vec![Command::Start],
            vec![Command::LeftPress, Command::RotateCw],
            vec![Command::LeftRelease, Command::SoftDropPress],
            vec![Command::SoftDropRelease, Command::HardDrop],
            vec![],
        ];
        let mut legacy = Game::new(0x5445_5452_4144);
        let mut configured = Game::with_config(0x5445_5452_4144, GameConfig::guideline_marathon());
        for commands in trace {
            let a = legacy.step(&commands).unwrap();
            let b = configured.step(&commands).unwrap();
            assert_eq!(a.events, b.events);
            assert_eq!(a.snapshot, b.snapshot);
            assert_eq!(legacy.state_hash(), configured.state_hash());
        }
    }
    #[test]
    fn state_hash_includes_hidden_simulation_state() {
        let mut a = Game::new(7);
        let mut b = a.clone();
        b.gravity = 1;
        assert_ne!(a.state_hash(), b.state_hash());
        a.step(&[Command::Start]).unwrap();
        b.step(&[Command::Start]).unwrap();
        assert_ne!(a.state_hash(), b.state_hash());
    }
    #[test]
    fn horizontal_input_during_clear_moves_the_next_piece_on_spawn() {
        let mut game = Game::new(7);
        game.phase = Phase::Clearing;
        let mut events = Vec::new();
        game.command(Command::LeftPress, &mut events);
        assert!(events.is_empty());
        assert_eq!(game.spawn_buffer, Some(-1));

        game.activate(PieceKind::T, &mut events);
        assert_eq!(game.active.x, 2);
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::Spawned { .. },
                GameEvent::Moved { dx: -1, dy: 0 }
            ]
        ));
    }
    #[test]
    fn releasing_buffered_horizontal_input_cancels_the_spawn_move() {
        let mut game = Game::new(7);
        game.phase = Phase::Clearing;
        let mut events = Vec::new();
        game.command(Command::RightPress, &mut events);
        game.command(Command::RightRelease, &mut events);
        game.activate(PieceKind::T, &mut events);
        assert_eq!(game.active.x, 3);
        assert!(matches!(events.as_slice(), [GameEvent::Spawned { .. }]));
    }
}
