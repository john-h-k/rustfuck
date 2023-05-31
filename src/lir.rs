use std::{
    collections::HashMap,
    io::{self, Read, Write},
};

use anyhow::Result;
use log::{info, trace};
use tap::prelude::*;

use crate::{
    hir::HirOp,
    ir::IrLike,
    state::{add_offset_8, add_offset_size, BrainfuckState},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LirOp<'a> {
    Modify(isize),
    Move(isize),

    WriteZero,       // Zeroes the current cell
    Hop(isize),      // Moves +/- in hops of n.0 until it finds a non-zero cell
    MoveCell(isize), // Adds the content of the current cell to another cell

    In,
    Out,

    BrFor,
    BrBack,

    CnstMovSet(Vec<(/* offset */ isize, /* value to modify by */ isize)>),

    // Lets you insert comments into LIR
    Meta(&'a str),
}

impl IrLike for LirOp<'_> {
    fn to_compact(&self) -> String {
        match self {
            LirOp::Modify(delta) => {
                format!("Mod({delta})")
            }
            LirOp::Move(delta) => {
                format!("Mov({delta})")
            }
            LirOp::In => "In".into(),
            LirOp::Out => "Out".into(),
            LirOp::BrFor => "[Br->".into(),
            LirOp::BrBack => "<-Br]".into(),
            LirOp::WriteZero => "Zero".into(),
            LirOp::Hop(mov_delta) => format!("Hop({mov_delta})"),
            LirOp::MoveCell(delta) => format!("MovCell({delta})"),
            LirOp::CnstMovSet(set) => format!("CnstMovSet({set:?})"),
            LirOp::Meta(comment) => format!("<{comment}>"),
        }
    }
}

pub struct LirGen;

impl LirGen {
    pub fn gen_ir(hir: &[HirOp]) -> Vec<LirOp> {
        info!("Starting LIR gen");

        let mut pos = 0;

        let mut lir = Vec::new();

        while let Some(op) = hir.get(pos) {
            // Any combo like [-], [+], [++++] is a set-to-zero
            // (not really, but its either that or an infinite loop)

            let lir_op = match op {
                HirOp::Modify(delta) => LirOp::Modify(*delta),
                HirOp::Move(delta) => LirOp::Move(*delta),
                HirOp::In => LirOp::In,
                HirOp::Out => LirOp::Out,
                HirOp::BrFor => {
                    if let Some((opt, skip)) = Self::try_opt_simple_hir_loop(&hir[pos..]) {
                        trace!("applied HIR loop-opt {opt:?} (was {:?})", &opt);
                        pos += skip;
                        opt
                    } else {
                        LirOp::BrFor
                    }
                }
                HirOp::BrBack => LirOp::BrBack,
            };

            lir.push(lir_op);
            pos += 1;
        }

        // Second LIR pass

        let mut lir2 = Vec::new();
        let mut pos = 0;

        while let Some(op) = lir.get(pos) {
            // TODO: make less-allocy (vecs)
            let lir2_ops = match op {
                LirOp::BrFor => {
                    if let Some((opts, skip)) = Self::try_opt_simple_lir_loop(&lir[pos..]) {
                        trace!("applied LIR loop-opt {:?}", &opts);
                        pos += skip;
                        opts
                    } else {
                        vec![LirOp::BrFor]
                    }
                }
                op =>
                /* clone is cheap as we don't have any CnstMovSets yet */
                {
                    vec![op.clone()]
                }
            };

            lir2.extend(lir2_ops);
            pos += 1;
        }

        lir2
    }

    /// A simple loop is one with no nested loops
    fn try_opt_simple_hir_loop(hir: &[HirOp]) -> Option<(LirOp, usize)> {
        let loop_end = hir[1..]
            .iter()
            .position(|&op| matches!(op, HirOp::BrFor | HirOp::BrBack))
            .map(|v| /* account for skipping first br */ v + 1);

        let loop_end = match loop_end {
            None => return None,
            Some(loop_end) if hir[loop_end] == HirOp::BrFor /* nested loop, not simple */ => return None,
            Some(loop_end) => loop_end,
        };

        assert!(hir[0] == HirOp::BrFor && hir[loop_end] == HirOp::BrBack);
        let loop_content = &hir[1..loop_end];

        trace!("attempting HIR loop-opt for {loop_content:?}");

        match loop_content {
            [HirOp::Modify(_)] => Some((LirOp::WriteZero, 2)),
            [HirOp::Move(delta)] => Some((LirOp::Hop(*delta), 2)),
            [HirOp::Modify(-1), HirOp::Move(delta), HirOp::Modify(1), HirOp::Move(ndelta)]
                if *delta == -ndelta =>
            {
                Some((LirOp::MoveCell(*delta), 5))
            }
            _ => {
                trace!("missed HIR loop-opt for {:?}", loop_content.to_compact());
                None
            }
        }
    }

    fn try_opt_simple_lir_loop<'a>(lir: &[LirOp<'a>]) -> Option<(Vec<LirOp<'a>>, usize)> {
        let loop_end = lir[1..]
            .iter()
            .position(|op| matches!(op, LirOp::BrFor | LirOp::BrBack))
            .map(|v| /* account for skipping first br */ v + 1);

        let loop_end = match loop_end {
            None => return None,
            Some(loop_end) if lir[loop_end] == LirOp::BrFor /* nested loop, not simple */ => return None,
            Some(loop_end) => loop_end,
        };

        assert!(lir[0] == LirOp::BrFor && lir[loop_end] == LirOp::BrBack);
        let loop_content = &lir[1..loop_end];

        trace!("attempting LIR loop-opt for {loop_content:?}");

        if loop_content
            .iter()
            .all(|op| matches!(op, LirOp::Modify(_) | LirOp::Move(_)))
        {
            // mod/mov chain
            // we can transform this into a special node

            let mut set = Vec::new();

            let mut offset = 0isize;
            for op in loop_content {
                match op {
                    LirOp::Move(delta) => offset += delta,
                    LirOp::Modify(delta) => set.push((offset, *delta)),
                    _ => unreachable!(),
                }
            }

            // Key point - we must insert a mov to ensure we remain in the same location at the end
            let fixup_mov = LirOp::Move(offset);

            Some((
                vec![
                    LirOp::BrFor,
                    LirOp::CnstMovSet(set),
                    fixup_mov,
                    LirOp::BrBack,
                ],
                loop_content.len() + 1,
            ))
        } else {
            trace!("missed LIR loop-opt for {:?}", loop_content.to_compact());
            None
        }
    }
}

pub struct LirInterpreter;

impl LirInterpreter {
    pub fn execute(program: &[LirOp]) -> Result<()> {
        info!("Starting LIR interpreter");

        if cfg!(feature = "trace") {
            eprintln!("[Tracing enabled]");
        }

        let branch_table = Self::gen_branch_table(program)?;

        let mut instr_pointer = 0;
        let mut state = BrainfuckState::new();

        let mut stdin = io::stdin().lock();
        let mut stdout = io::stdout().lock();

        // Tracing is very simple, only handles non-nested loops
        #[derive(Debug)]
        struct Trace<'a> {
            loc: (/* start */ usize, /* end */ usize),
            hit_count: usize,
            ops: Vec<LirOp<'a>>,
        }
        let mut traces = HashMap::new();
        let mut last_trace = Vec::new();
        let mut last_trace_start = usize::MAX;

        while let Some(ref command) = program.get(instr_pointer) {
            if cfg!(feature = "trace") {
                match *command {
                    command @ LirOp::BrFor => {
                        last_trace.clear();
                        last_trace.push(command.clone());
                        last_trace_start = instr_pointer;
                    }
                    command @ LirOp::BrBack if last_trace_start != usize::MAX => {
                        last_trace.push(command.clone());
                        let loc = (last_trace_start, instr_pointer);
                        traces
                            .entry(loc)
                            .and_modify(|t: &mut Trace| t.hit_count += 1)
                            .or_insert(Trace {
                                loc,
                                hit_count: 1,
                                ops: last_trace,
                            });

                        last_trace_start = usize::MAX;
                        last_trace = Vec::new();
                    }
                    command if last_trace_start != usize::MAX => last_trace.push(command.clone()),
                    _ => {}
                }
            }

            match command {
                LirOp::Modify(delta) => {
                    state.modify_cur_cell_with(|c| {
                        add_offset_8(c, *delta as i8);
                    });
                }
                LirOp::Move(delta) => {
                    add_offset_size(&mut state.pos, *delta);
                }
                LirOp::Out => {
                    stdout
                        .write_all(&[state.read_cur_cell()])
                        .expect("writing to `stdout` failed");
                }
                LirOp::In => {
                    let mut buff = [0; 1];
                    stdin
                        .read_exact(&mut buff)
                        .expect("reading from `stdin` failed");

                    state.set_cur_cell(buff[0]);
                }
                LirOp::BrFor => {
                    if state.read_cur_cell() == 0 {
                        instr_pointer = branch_table[instr_pointer];

                        // If tracing, insert a meta node to indicate we skipped a branch here
                        // otherwise, complex loops where the inner loop was skipped will look like simple loops
                        if cfg!(feature = "trace") {
                            last_trace.push(LirOp::Meta("BR SKIP"))
                        }
                    }
                }
                LirOp::BrBack => {
                    if state.read_cur_cell() != 0 {
                        instr_pointer = branch_table[instr_pointer];
                    }
                }
                LirOp::WriteZero => state.set_cur_cell(0),
                LirOp::Hop(mov_delta) => {
                    while state.read_cur_cell() > 0 {
                        add_offset_size(&mut state.pos, *mov_delta);
                    }
                }
                LirOp::MoveCell(delta) => {
                    if state.read_cur_cell() != 0 {
                        let mut target = state.pos;
                        add_offset_size(&mut target, *delta);

                        state.set_cell(
                            state
                                .read_cell(target)
                                .overflowing_add(state.read_cur_cell())
                                .0,
                            target,
                        );
                        state.set_cur_cell(0);
                    }
                }
                LirOp::Meta(comment) => info!("META: {}", comment),
                LirOp::CnstMovSet(set) => {
                    for (offset, delta) in set {
                        let mut target = state.pos;
                        add_offset_size(&mut target, *offset);
                        let cur = state.read_cell(target);

                        let mut new = cur;
                        add_offset_8(&mut new, *delta as i8);

                        state.set_cell(new, target);
                    }
                }
            };

            instr_pointer += 1;
        }

        if cfg!(feature = "trace") {
            for (_, trace) in traces
                .iter()
                .collect::<Vec<_>>()
                .tap_mut(|v| v.sort_by_key(|(_, t)| t.hit_count))
                .tap_mut(|v| v.reverse())
            {
                if trace.ops.contains(&LirOp::Meta("BR SKIP")) {
                    // Not a simple loop, skip
                    continue;
                }

                eprintln!(
                    "Trace: hit_count={}, loc=({},{}), ops={}",
                    trace.hit_count,
                    trace.loc.0,
                    trace.loc.1,
                    trace.ops.to_compact()
                );
            }
        }

        Ok(())
    }

    fn gen_branch_table(program: &[LirOp]) -> Result<Vec<usize>> {
        let mut table = vec![0; program.len()];

        let mut instr_pointer = 0;

        while let Some(ref command) = program.get(instr_pointer) {
            if let LirOp::BrFor = command {
                let mut depth = 0;
                let mut pos = instr_pointer;

                loop {
                    pos += 1;

                    match program.get(pos) {
                        Some(LirOp::BrFor) => depth += 1,
                        Some(LirOp::BrBack) if depth > 0 => depth -= 1,
                        Some(LirOp::BrBack) => {
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
