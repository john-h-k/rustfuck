#[derive(Debug)]
pub struct BrainfuckState {
    pub cells: Vec<u8>,
    pub pos: usize,
}

impl BrainfuckState {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            pos: 0,
        }
    }

    pub fn read_cell(&self, i: usize) -> u8 {
        // If the cell is OOB, it cannot have been written to, so must be zero
        *self.cells.get(i).unwrap_or(&0u8)
    }

    pub fn read_cur_cell(&self) -> u8 {
        self.read_cell(self.pos)
    }

    pub fn set_cell(&mut self, val: u8, i: usize) {
        if i >= self.cells.len() {
            self.cells.resize(i + 1, 0);
        }

        self.cells[i] = val;
    }

    pub fn set_cur_cell(&mut self, val: u8) {
        self.set_cell(val, self.pos);
    }

    pub fn modify_cur_cell_with(&mut self, f: impl Fn(&mut u8)) {
        if self.pos >= self.cells.len() {
            self.cells.resize(self.pos + 1, 0);
        }

        // SAFETY: We have just resized to at least `self.pos`, so this is ok
        unsafe { f(self.cells.get_unchecked_mut(self.pos)) };
    }

    pub fn modify_cur_cell_by(&mut self, arg: i32) {
        if self.pos >= self.cells.len() {
            self.cells.resize(self.pos + 1, 0);
        }

        self.cells[self.pos] = (self.cells[self.pos] as i32 + arg) as u8;
    }
}
