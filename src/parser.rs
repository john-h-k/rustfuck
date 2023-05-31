use std::{
    env, fs,
    io::{self, Read, Write},
};

use anyhow::Result;

use crate::hir::BfOp;
use crate::state::BrainfuckState;

pub struct BfParser;

impl BfParser {
    pub fn parse(program: &[u8]) -> Result<Vec<BfOp>> {
        let mut ir = Vec::new();

        for command in program.iter() {
            match command {
                b'+' => ir.push(BfOp::Inc),
                b'-' => ir.push(BfOp::Dec),
                b'>' => ir.push(BfOp::MvRight),
                b'<' => ir.push(BfOp::MvLeft),
                b'.' => ir.push(BfOp::Out),
                b',' => ir.push(BfOp::In),
                b'[' => ir.push(BfOp::BrFor),
                b']' => ir.push(BfOp::BrBack),

                _ => continue,
            }
        }

        Ok(ir)
    }
}

pub struct BfInterpreter;

impl BfInterpreter {
    pub fn execute(program: &[BfOp]) -> Result<()> {
        let mut instr_pointer = 0;

        let mut state = BrainfuckState {
            cells: Vec::new(),
            pos: 0,
        };

        let mut stdout = io::stdout().lock();
        let mut stdin = io::stdin().lock();

        while let Some(command) = program.get(instr_pointer) {
            match *command {
                BfOp::MvRight => state.pos += 1,
                BfOp::MvLeft if state.pos == 0 => panic!(
                    "Tried to decrement data pointer below 0 at position {}",
                    instr_pointer
                ),
                BfOp::MvLeft => state.pos -= 1,
                BfOp::Inc => state.modify_cur_cell_by(1),
                BfOp::Dec => state.modify_cur_cell_by(-1),
                BfOp::Out => {
                    stdout
                        .write_all(&[state.read_cur_cell()])
                        .expect("writing to `stdout` failed");
                }
                BfOp::In => {
                    let mut buff = [0; 1];
                    stdin
                        .read_exact(&mut buff)
                        .expect("reading from `stdin` failed");

                    state.set_cur_cell(buff[0]);
                }

                BfOp::BrFor if state.read_cur_cell() == 0 => {
                    let mut depth = 0;
                    let mut pos = instr_pointer;

                    loop {
                        pos += 1;

                        match program.get(pos) {
                            Some(BfOp::BrFor) => depth += 1,
                            Some(BfOp::BrBack) if depth > 0 => depth -= 1,
                            Some(BfOp::BrBack) => {
                                // Reached the matching bracket
                                // The next instruction we want to execute is the one AFTER this,
                                // but we increment instr_pointer at the end of the loop
                                instr_pointer = pos;
                                break;
                            }
                            None => panic!("Unmatched '[' at position {}", instr_pointer),
                            _ => {}
                        }
                    }
                }

                BfOp::BrBack if state.read_cur_cell() != 0 => {
                    let mut depth = 0;
                    let mut pos = instr_pointer;

                    loop {
                        pos -= 1;

                        match program.get(pos) {
                            Some(BfOp::BrBack) => depth += 1,
                            Some(BfOp::BrFor) if depth > 0 => depth -= 1,
                            Some(BfOp::BrFor) => {
                                // Reached the matching bracket
                                // The next instruction we want to execute is the one AFTER this,
                                // but we increment instr_pointer at the end of the loop
                                instr_pointer = pos;
                                break;
                            }
                            None => panic!("Unmatched ']' at position {}", instr_pointer),
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
