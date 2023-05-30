use std::{str::CharIndices, io::{self, Write, Read}, ops::AddAssign};
use anyhow::Result;
use bumpalo::Bump;

pub struct IrGen {
    program: Vec<u8>,
}

/// Represents a "real" brainfuck operation before optimisation
#[derive(Debug, PartialEq, Eq)]
pub enum BfOp {
    Inc,
    Dec,
    MvRight,
    MvLeft,
    In,
    Out,

    BrFor,
    BrBack,
}

/// Represents operations after the first opt pass
/// * +- have been collapsed
/// * >< have been collapsed
#[derive(Debug, PartialEq, Eq)]
pub enum HirOp {
    Modify(isize), // add or subtract
    Move(isize), // left or right movement
    In,
    Out,

    BrFor,
    BrBack,
}

impl IrGen {
    pub fn new(program: Vec<u8>) -> Self {
        Self { program }
    }

    pub fn gen(&mut self) -> Vec<HirOp> {
        let mut ir = Vec::new();
        
        for command in self.program {
            match command {
                b'+' => ir.push(BfOp::Inc),
                b'-' => ir.push(BfOp::Dec),
                b'>' => ir.push(BfOp::MvRight),
                b'<' => ir.push(BfOp::MvLeft),
                b'.' => ir.push(BfOp::Out),
                b',' => ir.push(BfOp::In),
                b'[' => ir.push(BfOp::BrFor),
                b']' => ir.push(BfOp::BrBack)

                _ => continue
            }
        }

        Self::lower(ir)
    }

    fn lower(bf: &[BfOp]) -> Vec<HirOp> {
        let mut result = Vec::new();

        let iter = bf.iter().peekable();

        while let Some(bf_op) = iter.peek() {
            let hir_op = match bf_op {
                BfOp::Inc | BfOp::Dec => {
                    let mod_ops = iter.take_while(|op| matches!(op, BfOp::Inc | BfOp::Dec));

                    let delta = mod_ops.map(|&op| if op == BfOp::Inc { 1 } else { -1 }).sum();

                    HirOp::Modify(delta)
                },
                BfOp::MvRight | BfOp::MvLeft => {
                    let mod_ops = iter.take_while(|op| matches!(op, BfOp::MvRight | BfOp::MvLeft));

                    let delta = mod_ops.map(|&op| if op == BfOp::MvRight { 1 } else { -1 }).sum();

                    HirOp::Move(delta)
                },
                BfOp::In => HirOp::In,
                BfOp::Out => HirOp::Out,
                BfOp::BrFor => HirOp::BrFor,
                BfOp::BrBack => HirOp::BrBack,
            };

            result.push(hir_op);
        }

        result
    }
}


#[derive(Debug)]
pub struct BrainfuckState {
    pub cells: Vec<u8>,
    pub pos: usize,
}

impl BrainfuckState {
    pub fn new() -> Self {
        Self { cells: Vec::new(), pos: 0 }
    }
    
    pub fn read_cell(&self, i: usize) -> u8 {
        // If the cell is OOB, it cannot have been written to, so must be zero
        *self.cells.get(i).unwrap_or(&0u8)
    }

    pub fn read_cur_cell(&self) -> u8 {
        self.read_cell(self.pos)
    }

    fn set_cur_cell(&mut self, val: u8) {
        if self.pos >= self.cells.len() {
            self.cells.resize(self.pos + 1, 0);
        }

        self.cells[self.pos] = val;
    }

    fn modify_cur_cell_with(&mut self, f: impl Fn(&mut u8)) {
        if self.pos >= self.cells.len() {
            self.cells.resize(self.pos + 1, 0);
        }

        f(&mut self.cells[self.pos]);        
    }
}

fn add_offset(dst: &mut impl AddAssign<Rhs=usize>, delta: isize) -> &mut usize {
    if delta > 0 {
        *dst += delta as usize
    } else {
        *dst += delta.abs() as usize
    };

    dst
}

pub struct HirInterpreter;

impl HirInterpreter {
    pub fn new() -> Self {
        Self
    }
    
    pub fn execute(program: &[HirOp]) -> Result<()> {
        let branch_table = Self::gen_branch_table(program)?;

        let mut instr_pointer = 0;
        let mut state = BrainfuckState::new();

        let mut stdin = io::stdin().lock();
        let mut stdout = io::stdout().lock();
        
        while let Some(&command) = program.get(instr_pointer) {
            match command {
                HirOp::Modify(delta) => { state.modify_cur_cell_with(|c| { add_offset(c, delta); }); },
                HirOp::Move(delta) => { add_offset(&mut state.pos, delta); },
                HirOp::Out => {
                    stdout
                        .write_all(&[state.read_cur_cell()])
                        .expect("writing to `stdout` failed");
                }
                HirOp::In => {
                    let mut buff = [0; 1];
                    stdin
                        .read_exact(&mut buff)
                        .expect("reading from `stdin` failed");

                    state.set_cur_cell(buff[0]);
                }
                HirOp::BrFor if state.read_cur_cell() == 0 => {
                    instr_pointer = branch_table[instr_pointer];
                }
                HirOp::BrBack if state.read_cur_cell() != 0 => {
                    instr_pointer = branch_table[instr_pointer];
                }
                _ => {}
            };
        }

        Ok(())
    }

    fn gen_branch_table(program: &[HirOp]) -> Result<Vec<usize>> {
        let mut table = vec![0; program.len()];

        let mut instr_pointer = 0;

        while let Some(&command) = program.get(instr_pointer) {
            if let HirOp::BrFor = command {
                let mut depth = 0;
                let mut pos = instr_pointer;

                loop {
                    pos += 1;

                    match program.get(pos) {
                        Some(HirOp::BrFor) => depth += 1,
                        Some(HirOp::BrBack) if depth > 0 => depth -= 1,
                        Some(HirOp::BrBack) => {
                            table[instr_pointer] = pos;
                            table[pos] = instr_pointer;

                            break;
                        }
                        None => unreachable!("Unterminated bracket, should've been caught earlier"),
                        _ => {}
                    }
                }
            }

            instr_pointer += 1;
        }

        Ok(table)
    }
}