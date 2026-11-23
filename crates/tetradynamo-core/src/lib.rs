//! Deterministic falling-block simulation for Tetra|D.

use std::collections::VecDeque;

pub const WIDTH: i8 = 10;
pub const HEIGHT: i8 = 22;
pub const HIDDEN_ROWS: i8 = 2;
pub const TICK_HZ: u32 = 1_000;
const LOCK_DELAY: u32 = 500;
const CLEAR_DELAY: u32 = 500;
const CLEAR_NOTICE_DELAY: u32 = 1_000;
const DAS: u32 = 167;
const ARR: u32 = 33;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// The recognized T-spin type on the piece that locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spin {
    Full,
    Mini,
}

/// Presentation data emitted when a line clear is locked in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearNotice {
    pub clear: Clear,
    pub points: u32,
    pub spin: Option<Spin>,
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
pub struct Board {
    cells: [[Option<PieceKind>; WIDTH as usize]; HEIGHT as usize],
}
impl Default for Board {
    fn default() -> Self {
        Self {
            cells: [[None; WIDTH as usize]; HEIGHT as usize],
        }
    }
}
impl Board {
    pub fn cell(&self, x: i8, y: i8) -> Option<PieceKind> {
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            self.cells[y as usize][x as usize]
        } else {
            None
        }
    }
    fn collides(&self, p: &ActivePiece, dx: i8, dy: i8, rot: u8) -> bool {
        cells(p.kind, rot).iter().any(|&(cx, cy)| {
            let x = p.x + dx + cx;
            let y = p.y + dy + cy;
            !(0..WIDTH).contains(&x)
                || y >= HEIGHT
                || (y >= 0 && self.cells[y as usize][x as usize].is_some())
        })
    }
    fn lock(&mut self, p: &ActivePiece) {
        for &(cx, cy) in cells(p.kind, p.rotation) {
            let (x, y) = (p.x + cx, p.y + cy);
            if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
                self.cells[y as usize][x as usize] = Some(p.kind);
            }
        }
    }
    fn full_rows(&self) -> Vec<usize> {
        (0..HEIGHT as usize)
            .filter(|&y| self.cells[y].iter().all(Option::is_some))
            .collect()
    }
    fn remove(&mut self, rows: &[usize]) {
        let mut kept: Vec<_> = (0..HEIGHT as usize)
            .filter(|y| !rows.contains(y))
            .map(|y| self.cells[y])
            .collect();
        while kept.len() < HEIGHT as usize {
            kept.insert(0, [None; WIDTH as usize]);
        }
        self.cells.copy_from_slice(&kept);
    }
    fn is_empty_after_removal(&self, rows: &[usize]) -> bool {
        self.cells
            .iter()
            .enumerate()
            .filter(|(y, _)| !rows.contains(y))
            .all(|(_, row)| row.iter().all(Option::is_none))
    }
    fn occupied_or_wall(&self, x: i8, y: i8) -> bool {
        !(0..WIDTH).contains(&x)
            || !(0..HEIGHT).contains(&y)
            || self.cells[y as usize][x as usize].is_some()
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub board: Board,
    pub active: ActivePiece,
    pub queue: VecDeque<PieceKind>,
    pub hold: Option<PieceKind>,
    pub phase: Phase,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    pub elapsed_ticks: u64,
    pub clear_notice: Option<ClearNotice>,
    pub combo: u32,
    pub back_to_back: bool,
    seed: u64,
    gravity: u32,
    lock: u32,
    clear_timer: u32,
    clear_notice_timer: u32,
    pending_rows: Vec<usize>,
    hold_used: bool,
    left: bool,
    right: bool,
    soft: bool,
    horizontal: u32,
    lock_resets: u8,
}
impl Game {
    pub fn new(seed: u64) -> Self {
        let mut g = Self {
            board: Board::default(),
            active: spawn(PieceKind::T),
            queue: VecDeque::new(),
            hold: None,
            phase: Phase::Title,
            score: 0,
            lines: 0,
            level: 1,
            elapsed_ticks: 0,
            clear_notice: None,
            combo: 0,
            back_to_back: false,
            seed,
            gravity: 0,
            lock: 0,
            clear_timer: 0,
            clear_notice_timer: 0,
            pending_rows: vec![],
            hold_used: false,
            left: false,
            right: false,
            soft: false,
            horizontal: 0,
            lock_resets: 0,
        };
        g.fill_queue();
        g.active = spawn(g.take());
        g
    }
    fn random(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    fn fill_queue(&mut self) {
        while self.queue.len() < 7 {
            let mut bag = PieceKind::ALL;
            for i in (1..7).rev() {
                let j = (self.random() % (i as u64 + 1)) as usize;
                bag.swap(i, j);
            }
            self.queue.extend(bag);
        }
    }
    fn take(&mut self) -> PieceKind {
        self.fill_queue();
        self.queue.pop_front().unwrap()
    }
    pub fn next(&self) -> impl Iterator<Item = PieceKind> + '_ {
        self.queue.iter().copied()
    }
    pub fn gravity_cps(&self) -> u32 {
        (1_000 / gravity_ms(self.level)).max(1)
    }
    pub fn ghost_y(&self) -> i8 {
        let mut d = 0;
        while !self
            .board
            .collides(&self.active, 0, d + 1, self.active.rotation)
        {
            d += 1;
        }
        self.active.y + d
    }
    /// Rows currently in the 500 ms line-clear animation, with elapsed ticks.
    pub fn clearing_animation(&self) -> Option<(&[usize], u32)> {
        (self.phase == Phase::Clearing).then_some((&self.pending_rows, self.clear_timer))
    }
    pub fn step(&mut self, command: Option<Command>) {
        if let Some(c) = command {
            self.command(c);
        }
        if self.clear_notice_timer > 0 {
            self.clear_notice_timer -= 1;
            if self.clear_notice_timer == 0 {
                self.clear_notice = None;
            }
        }
        if self.phase == Phase::Clearing {
            self.clear_timer += 1;
            if self.clear_timer >= CLEAR_DELAY {
                self.board.remove(&self.pending_rows);
                self.lines += self.pending_rows.len() as u32;
                self.level = (1 + self.lines / 10).min(15);
                self.spawn_next();
            }
            return;
        }
        if self.phase != Phase::Falling {
            return;
        }
        self.elapsed_ticks += 1;
        self.horizontal_step();
        let interval = if self.soft {
            (gravity_ms(self.level) / 20).max(17)
        } else {
            gravity_ms(self.level)
        };
        self.gravity += 1;
        if self.gravity >= interval {
            self.gravity -= interval;
            self.down(false);
        }
        if self
            .board
            .collides(&self.active, 0, 1, self.active.rotation)
        {
            self.lock += 1;
            if self.lock >= LOCK_DELAY {
                self.lock_piece();
            }
        } else {
            self.lock = 0;
        }
    }
    fn command(&mut self, c: Command) {
        match c {
            Command::Start if self.phase == Phase::Title => self.phase = Phase::Falling,
            Command::Restart if matches!(self.phase, Phase::GameOver | Phase::Complete) => {
                *self = Self::new(self.seed)
            }
            Command::Pause if self.phase == Phase::Falling => self.phase = Phase::Paused,
            Command::Pause if self.phase == Phase::Paused => self.phase = Phase::Falling,
            Command::LeftPress => {
                self.left = true;
                self.horizontal = 0;
                self.move_x(-1)
            }
            Command::RightPress => {
                self.right = true;
                self.horizontal = 0;
                self.move_x(1)
            }
            Command::LeftRelease => self.left = false,
            Command::RightRelease => self.right = false,
            Command::SoftDropPress => {
                self.soft = true;
                self.down(true)
            }
            Command::SoftDropRelease => self.soft = false,
            Command::RotateCw => self.rotate(1),
            Command::RotateCcw => self.rotate(3),
            Command::HardDrop => {
                if self.phase == Phase::Falling {
                    let y = self.ghost_y();
                    let d = (y - self.active.y) as u64;
                    self.active.y = y;
                    self.score += d * 2;
                    self.lock_piece();
                }
            }
            Command::Hold => self.hold(),
            _ => {}
        }
    }
    fn horizontal_step(&mut self) {
        let dir = if self.left {
            Some(-1)
        } else if self.right {
            Some(1)
        } else {
            None
        };
        let Some(dir) = dir else { return };
        self.horizontal += 1;
        if self.horizontal == DAS
            || (self.horizontal > DAS && (self.horizontal - DAS).is_multiple_of(ARR))
        {
            self.move_x(dir);
        }
    }
    fn move_x(&mut self, dx: i8) {
        if self.phase == Phase::Falling
            && !self
                .board
                .collides(&self.active, dx, 0, self.active.rotation)
        {
            self.active.x += dx;
            self.active.last_rotation = false;
            self.reset_lock();
        }
    }
    fn down(&mut self, points: bool) {
        if !self
            .board
            .collides(&self.active, 0, 1, self.active.rotation)
        {
            self.active.y += 1;
            if points {
                self.score += 1
            };
            self.lock = 0;
        }
    }
    fn rotate(&mut self, dir: u8) {
        if self.phase != Phase::Falling {
            return;
        }
        let next = (self.active.rotation + dir) % 4;
        for (dx, dy) in kicks(self.active.kind, self.active.rotation, next) {
            if !self.board.collides(&self.active, *dx, *dy, next) {
                self.active.x += dx;
                self.active.y += dy;
                self.active.rotation = next;
                self.active.last_rotation = true;
                self.active.last_kick = (*dx, *dy);
                self.reset_lock();
                break;
            }
        }
    }
    fn reset_lock(&mut self) {
        if self
            .board
            .collides(&self.active, 0, 1, self.active.rotation)
            && self.lock_resets < 15
        {
            self.lock = 0;
            self.lock_resets += 1;
        }
    }
    fn hold(&mut self) {
        if self.phase != Phase::Falling || self.hold_used {
            return;
        }
        self.hold_used = true;
        let current = self.active.kind;
        self.active = spawn(match self.hold.replace(current) {
            Some(k) => k,
            None => self.take(),
        });
        if self
            .board
            .collides(&self.active, 0, 0, self.active.rotation)
        {
            self.phase = Phase::GameOver;
        }
    }
    fn lock_piece(&mut self) {
        let spin = self.t_spin();
        self.board.lock(&self.active);
        self.lock = 0;
        self.lock_resets = 0;
        let rows = self.board.full_rows();
        if rows.is_empty() {
            self.combo = 0;
            self.spawn_next();
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
        let gained = score_clear(
            n,
            self.level,
            spin.is_some(),
            spin == Some(Spin::Mini),
            self.combo,
            self.back_to_back,
            self.board.is_empty_after_removal(&rows),
        );
        self.score += gained as u64;
        self.back_to_back = spin.is_some() || n == 4;
        self.clear_notice = Some(ClearNotice {
            clear,
            points: gained,
            spin,
        });
        self.clear_notice_timer = CLEAR_NOTICE_DELAY;
        self.pending_rows = rows;
        self.clear_timer = 0;
        self.phase = Phase::Clearing;
    }

    fn t_spin(&self) -> Option<Spin> {
        if self.active.kind != PieceKind::T || !self.active.last_rotation {
            return None;
        }
        let x = self.active.x + 1;
        let y = self.active.y + 1;
        let corners = [
            (x - 1, y - 1),
            (x + 1, y - 1),
            (x - 1, y + 1),
            (x + 1, y + 1),
        ];
        if corners
            .iter()
            .filter(|&&(x, y)| self.board.occupied_or_wall(x, y))
            .count()
            < 3
        {
            return None;
        }
        let front = match self.active.rotation {
            0 => [corners[0], corners[1]],
            1 => [corners[1], corners[3]],
            2 => [corners[2], corners[3]],
            _ => [corners[0], corners[2]],
        };
        if self.active.last_kick.1.abs() >= 2
            || front
                .iter()
                .all(|&(x, y)| self.board.occupied_or_wall(x, y))
        {
            Some(Spin::Full)
        } else {
            Some(Spin::Mini)
        }
    }
    fn spawn_next(&mut self) {
        if self.lines >= 150 {
            self.phase = Phase::Complete;
            return;
        }
        self.active = spawn(self.take());
        self.hold_used = false;
        self.gravity = 0;
        self.phase = if self
            .board
            .collides(&self.active, 0, 0, self.active.rotation)
        {
            Phase::GameOver
        } else {
            Phase::Falling
        };
    }
}

fn score_clear(
    lines: u32,
    level: u32,
    t_spin: bool,
    mini: bool,
    combo: u32,
    b2b: bool,
    perfect: bool,
) -> u32 {
    let mut base = if t_spin {
        [400, 800, 1200, 1600][lines as usize]
    } else {
        [0, 100, 300, 500, 800][lines as usize]
    };
    if t_spin && mini {
        base = 100 + [0, 100, 300, 0][lines as usize];
    }
    let mut points = base * level;
    if b2b && (t_spin || lines == 4) {
        points = points * 3 / 2;
    }
    if combo >= 2 {
        points += 50 * (combo - 1) * level;
    }
    if perfect {
        points += [0, 800, 1000, 1800, 2000][lines as usize] * level;
    }
    points
}
fn gravity_ms(level: u32) -> u32 {
    let l = level.max(1) as f64;
    ((0.8 - (l - 1.0) * 0.007).powf(l - 1.0) * 1000.0).round() as u32
}
fn spawn(kind: PieceKind) -> ActivePiece {
    ActivePiece {
        kind,
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
fn cells(k: PieceKind, r: u8) -> &'static [(i8, i8); 4] {
    shape_cells(k, r)
}
fn kicks(k: PieceKind, from: u8, to: u8) -> &'static [(i8, i8)] {
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
    match (k == PieceKind::I, from, to) {
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
    fn seeded_queue_is_a_bag() {
        let g = Game::new(4);
        let mut v: Vec<_> = g.next().take(6).collect();
        v.push(g.active.kind);
        assert_eq!(v.len(), 7);
        assert!(PieceKind::ALL.iter().all(|k| v.contains(k)));
    }
    #[test]
    fn title_starts_game() {
        let mut g = Game::new(1);
        g.step(Some(Command::Start));
        assert_eq!(g.phase, Phase::Falling);
    }
    #[test]
    fn hard_drop_locks() {
        let mut g = Game::new(1);
        g.step(Some(Command::Start));
        g.step(Some(Command::HardDrop));
        assert!(g.board.cells.iter().flatten().any(Option::is_some));
    }

    #[test]
    fn guideline_scoring_includes_combo_b2b_and_perfect_clear() {
        assert_eq!(score_clear(1, 1, false, false, 1, false, false), 100);
        assert_eq!(score_clear(4, 1, false, false, 2, true, false), 1250);
        assert_eq!(score_clear(4, 1, false, false, 1, false, true), 2800);
        assert_eq!(score_clear(2, 1, true, false, 1, false, false), 1200);
    }

    #[test]
    fn custom_clear_names_are_stable() {
        assert_eq!(Clear::Monad.label(), "MONAD");
        assert_eq!(Clear::Dyad.label(), "DYAD");
        assert_eq!(Clear::Triad.label(), "TRIAD");
        assert_eq!(Clear::TetraD.label(), "TETRA|D");
    }

    #[test]
    fn detects_full_and_mini_t_spins_from_corner_occupancy() {
        let piece = ActivePiece {
            kind: PieceKind::T,
            rotation: 0,
            x: 3,
            y: 5,
            last_rotation: true,
            last_kick: (0, 0),
        };
        let mut full = Game::new(1);
        full.active = piece;
        for (x, y) in [(3, 5), (5, 5), (3, 7)] {
            full.board.cells[y][x] = Some(PieceKind::I);
        }
        assert_eq!(full.t_spin(), Some(Spin::Full));

        let mut mini = Game::new(1);
        mini.active = piece;
        for (x, y) in [(3, 5), (3, 7), (5, 7)] {
            mini.board.cells[y][x] = Some(PieceKind::I);
        }
        assert_eq!(mini.t_spin(), Some(Spin::Mini));
    }
}
