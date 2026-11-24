//! Fixed Guideline board storage, collision, and line operations.

use crate::{shape_cells, ActivePiece, PieceKind, HEIGHT, WIDTH};

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

    pub fn cells(&self) -> &[[Option<PieceKind>; WIDTH as usize]; HEIGHT as usize] {
        &self.cells
    }

    pub(crate) fn hit(&self, p: &ActivePiece, dx: i8, dy: i8, r: u8) -> bool {
        shape_cells(p.kind, r).iter().any(|&(cx, cy)| {
            let (x, y) = (p.x + dx + cx, p.y + dy + cy);
            !(0..WIDTH).contains(&x)
                || y >= HEIGHT
                || (y >= 0 && self.cells[y as usize][x as usize].is_some())
        })
    }

    pub(crate) fn lock(&mut self, p: &ActivePiece) {
        for &(dx, dy) in shape_cells(p.kind, p.rotation) {
            let (x, y) = (p.x + dx, p.y + dy);
            if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
                self.cells[y as usize][x as usize] = Some(p.kind);
            }
        }
    }

    pub(crate) fn full(&self) -> Vec<usize> {
        (0..HEIGHT as usize)
            .filter(|&y| self.cells[y].iter().all(Option::is_some))
            .collect()
    }

    pub(crate) fn remove(&mut self, rows: &[usize]) {
        let mut remaining: Vec<_> = (0..HEIGHT as usize)
            .filter(|y| !rows.contains(y))
            .map(|y| self.cells[y])
            .collect();
        while remaining.len() < HEIGHT as usize {
            remaining.insert(0, [None; WIDTH as usize]);
        }
        self.cells.copy_from_slice(&remaining);
    }

    pub(crate) fn empty_after(&self, rows: &[usize]) -> bool {
        self.cells
            .iter()
            .enumerate()
            .filter(|(y, _)| !rows.contains(y))
            .all(|(_, row)| row.iter().all(Option::is_none))
    }

    pub(crate) fn occupied(&self, x: i8, y: i8) -> bool {
        !(0..WIDTH).contains(&x)
            || !(0..HEIGHT).contains(&y)
            || self.cells[y as usize][x as usize].is_some()
    }
}
