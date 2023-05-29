use anyhow::Result;
use std::io::{self, Read, Write};

pub struct Interpreter<'a> {
    program: &'a str,
}

struct BrainfuckState {
    cells: Vec<u8>,
    pos: usize,
}

#[derive(PartialEq, Eq)]
enum CellOp {
    Inc,
    Dec,
}

impl BrainfuckState {
    pub fn read_cur_cell(&self) -> u8 {
        // If the cell is OOB, it cannot have been written to, so must be zero
        *self.cells.get(self.pos).unwrap_or(&0u8)
    }

    fn set_cur_cell(&mut self, val: u8) {
        if self.pos >= self.cells.len() {
            self.cells.resize(self.pos + 1, 0);
        }

        self.cells[self.pos] = val;
    }

    fn modify_cur_cell(&mut self, op: CellOp) {
        if op == CellOp::Inc {
            self.set_cur_cell(self.read_cur_cell() + 1);
        } else {
            self.set_cur_cell(self.read_cur_cell() - 1);
        }
    }
}

impl<'a> Interpreter<'a> {
    pub fn new(program: &'a str) -> Self {
        Self { program }
    }

    pub fn execute(&self) -> Result<()> {
        let program = self.program.as_bytes();
        let mut instr_pointer = 0;

        let mut state = BrainfuckState {
            cells: Vec::new(),
            pos: 0,
        };

        let mut stdout = io::stdout().lock();
        let mut stdin = io::stdin().lock();

        while let Some(command) = program.get(instr_pointer) {
            match *command {
                b'>' => state.pos += 1,
                b'<' => state.pos -= 1,
                b'+' => state.modify_cur_cell(CellOp::Inc),
                b'-' => state.modify_cur_cell(CellOp::Dec),
                b'.' => {
                    stdout
                        .write_all(&[state.read_cur_cell()])
                        .expect("writing to `stdout` failed");
                }
                b',' => {
                    let mut buff = [0; 1];
                    stdin
                        .read_exact(&mut buff)
                        .expect("reading from `stdin` failed");

                    state.set_cur_cell(buff[0]);
                }

                b'[' if state.read_cur_cell() == 0 => {
                    let mut depth = 0;
                    let mut pos = instr_pointer;

                    loop {
                        pos += 1;

                        match program.get(pos) {
                            Some(b'[') => depth += 1,
                            Some(b']') if depth > 0 => depth -= 1,
                            Some(b']') => {
                                // Reached the matching bracket
                                // The next instruction we want to execute is the one AFTER this,
                                // but we increment instr_pointer at the end of the loop
                                instr_pointer = pos;
                                break;
                            }
                            _ => {}
                        }
                    }
                }

                b']' if state.read_cur_cell() != 0 => {
                    let mut depth = 0;
                    let mut pos = instr_pointer;

                    loop {
                        pos -= 1;

                        match program.get(pos) {
                            Some(b']') => depth += 1,
                            Some(b'[') if depth > 0 => depth -= 1,
                            Some(b'[') => {
                                // Reached the matching bracket
                                // The next instruction we want to execute is the one AFTER this,
                                // but we increment instr_pointer at the end of the loop
                                instr_pointer = pos;
                                break;
                            }
                            _ => {}
                        }
                    }
                }

                _ => {}
            }

            instr_pointer += 1;
        }

        Ok(())
    }
}
