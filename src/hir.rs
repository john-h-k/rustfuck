use std::{
    collections::HashMap,
    io::{self, Read, Write},
    ops::AddAssign,
    str::CharIndices,
};

use anyhow::Result;
use bumpalo::Bump;
use log::info;
use tap::prelude::*;

use crate::{
    ir::IrLike,
    state::{add_offset_8, add_offset_size, BrainfuckState},
};

pub struct HirGen {
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
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum HirOp {
    Modify(isize), // add or subtract
    Move(isize),   // left or right movement
    In,
    Out,
    BrFor,
    BrBack,
}

impl IrLike for HirOp {
    fn to_compact(&self) -> String {
        match self {
            HirOp::Modify(delta) => {
                if *delta > 0 {
                    "+".repeat(*delta as usize)
                } else {
                    "-".repeat(delta.unsigned_abs())
                }
            }
            HirOp::Move(delta) => {
                if *delta > 0 {
                    ">".repeat(*delta as usize)
                } else {
                    "<".repeat(delta.unsigned_abs())
                }
            }
            HirOp::In => ",".into(),
            HirOp::Out => ".".into(),
            HirOp::BrFor => "[".into(),
            HirOp::BrBack => "]".into(),
            _ => "!".into(),
        }
    }
}

impl HirGen {
    pub fn new(program: Vec<u8>) -> Self {
        Self { program }
    }

    pub fn gen(&mut self) -> Vec<HirOp> {
        info!("Starting HIR gen");

        let mut ir = Vec::new();

        for command in self.program.iter() {
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

        Self::lower(&ir)
    }

    fn lower(bf: &[BfOp]) -> Vec<HirOp> {
        let mut result = Vec::new();

        let mut pos = 0;

        while let Some(bf_op) = bf.get(pos) {
            let hir_op = match bf_op {
                BfOp::Inc | BfOp::Dec => {
                    let mod_ops = bf[pos..]
                        .iter()
                        .take_while(|op| matches!(op, BfOp::Inc | BfOp::Dec));

                    let delta = mod_ops
                        .map(|op| {
                            pos += 1;
                            if *op == BfOp::Inc {
                                1
                            } else {
                                -1
                            }
                        })
                        .sum();

                    HirOp::Modify(delta)
                }
                BfOp::MvRight | BfOp::MvLeft => {
                    let mod_ops = bf[pos..]
                        .iter()
                        .take_while(|op| matches!(op, BfOp::MvRight | BfOp::MvLeft));

                    let delta = mod_ops
                        .map(|op| {
                            pos += 1;
                            if *op == BfOp::MvRight {
                                1
                            } else {
                                -1
                            }
                        })
                        .sum();

                    HirOp::Move(delta)
                }
                BfOp::In => {
                    pos += 1;
                    HirOp::In
                }
                BfOp::Out => {
                    pos += 1;
                    HirOp::Out
                }
                BfOp::BrFor => {
                    pos += 1;
                    HirOp::BrFor
                }
                BfOp::BrBack => {
                    pos += 1;
                    HirOp::BrBack
                }
            };

            result.push(hir_op);
        }

        result
    }
}

pub struct HirInterpreter;

impl HirInterpreter {
    pub fn execute(program: &[HirOp]) -> Result<()> {
        if cfg!(trace) {
            eprintln!("[Tracing enabled]");
        }

        let branch_table = Self::gen_branch_table(program)?;

        let mut instr_pointer = 0;
        let mut state = BrainfuckState::new();

        let mut stdin = io::stdin().lock();
        let mut stdout = io::stdout().lock();

        // Tracing is very simple, only handles non-nested loops
        #[derive(Debug)]
        struct Trace {
            loc: (/* start */ usize, /* end */ usize),
            hit_count: usize,
            ops: Vec<HirOp>,
        }
        let mut traces = HashMap::new();
        let mut last_trace = Vec::new();
        let mut last_trace_start = usize::MAX;

        while let Some(ref command) = program.get(instr_pointer) {
            if cfg!(trace) {
                match command {
                    HirOp::BrFor => {
                        last_trace = Vec::new();
                        last_trace_start = instr_pointer;
                    }
                    HirOp::BrBack if !last_trace.is_empty() => {
                        let loc = (last_trace_start, instr_pointer);
                        traces
                            .entry(loc)
                            .and_modify(|t: &mut Trace| t.hit_count += 1)
                            .or_insert(Trace {
                                loc,
                                hit_count: 1,
                                ops: last_trace,
                            });

                        last_trace = Vec::new();
                    }
                    &&command => last_trace.push(command),
                }
            }

            match command {
                HirOp::Modify(delta) => {
                    state.modify_cur_cell_with(|c| {
                        add_offset_8(c, *delta as i8);
                    });
                }
                HirOp::Move(delta) => {
                    add_offset_size(&mut state.pos, *delta);
                }
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
                HirOp::BrFor => {
                    if state.read_cur_cell() == 0 {
                        instr_pointer = branch_table[instr_pointer];
                    }
                }
                HirOp::BrBack => {
                    if state.read_cur_cell() != 0 {
                        instr_pointer = branch_table[instr_pointer];
                    }
                }
            };

            instr_pointer += 1;
        }

        if cfg!(trace) {
            for (_, trace) in traces
                .iter()
                .collect::<Vec<_>>()
                .tap_mut(|v| v.sort_by_key(|(_, t)| t.hit_count))
                .tap_mut(|v| v.reverse())
            {
                let mut op_str = String::new();

                for op in trace.ops.iter() {
                    let stringified = match op {
                        HirOp::Modify(delta) => {
                            if *delta > 0 {
                                "+".repeat(*delta as usize)
                            } else {
                                "-".repeat(delta.unsigned_abs())
                            }
                        }
                        HirOp::Move(delta) => {
                            if *delta > 0 {
                                ">".repeat(*delta as usize)
                            } else {
                                "<".repeat(delta.unsigned_abs())
                            }
                        }
                        HirOp::In => ",".into(),
                        HirOp::Out => ".".into(),
                        HirOp::BrFor => "[".into(),
                        HirOp::BrBack => "]".into(),
                    };

                    op_str.push_str(&stringified);
                }

                eprintln!(
                    "Trace: hit_count={}, loc=({},{}), ops={}",
                    trace.hit_count, trace.loc.0, trace.loc.1, op_str
                );
            }
        }

        Ok(())
    }

    fn gen_branch_table(program: &[HirOp]) -> Result<Vec<usize>> {
        let mut table = vec![0; program.len()];

        let mut instr_pointer = 0;

        while let Some(ref command) = program.get(instr_pointer) {
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
