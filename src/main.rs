#![feature(let_chains)]

use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use clap::Parser;

use crate::{
    hir::{HirGen, HirInterpreter},
    jit::Jit,
    lir::{LirGen, LirInterpreter},
    parser::{BfInterpreter, BfParser},
};

mod hir;
mod ir;
mod jit;
mod lir;
mod parser;
mod state;

#[derive(Parser)]
#[command(name = "rustfuck")]
#[command(author = "John Harry Kelly <johnharrykelly@gmail.com>")]
#[command(version = "1.0")]
#[command(
    help_template = "{name}: {about-section}Version: {version}\nWritten by {author-with-newline}\n{usage-heading} {usage}\n{all-args} {tab}"
)]
#[command(about, long_about = None)]
#[command(group(
    clap::ArgGroup::new("backend")
        .required(true)
        .args(&["bf", "hir", "lir", "jit"]),
))]
struct Args {
    /// The file to execute
    /// If not provided, will enter REPL mode
    file: Option<PathBuf>,

    #[arg(long)]
    bf: bool,

    #[arg(long)]
    hir: bool,

    #[arg(long)]
    lir: bool,

    #[arg(long)]
    jit: bool,

    /// Provide profiling information
    #[arg(short, long)]
    profile: bool,

    /// How many times to run the resultant program (for profiling)
    #[arg(short, long, default_value_t = 1)]
    repeat: u32,
}

fn main() -> Result<()> {
    if env::var("RAW_PANIC").is_err() {
        human_panic::setup_panic!();
    }

    env_logger::init();

    let args = Args::parse();

    let content = fs::read_to_string(args.file.expect("repl disabled"))?;
    let content = Vec::from(content.as_bytes());

    let (duration, parsed) = run_once(|| BfParser::parse(&content));
    let parsed = parsed?;

    if args.profile {
        println!("Parse took {:?}", duration);
    }

    let duration = if args.bf {
        run_n(args.repeat, || BfInterpreter::execute(&parsed))
    } else {
        let (duration, hir) = run_once(|| HirGen::gen(&parsed));

        if args.profile {
            println!("HIR gen took {:?}", duration);
        }

        if args.hir {
            run_n(args.repeat, || HirInterpreter::execute(&hir))
        } else {
            let (duration, lir) = run_once(|| LirGen::gen_ir(&hir));

            if args.profile {
                println!("LIR gen took {:?}", duration);
            }

            if args.lir {
                run_n(args.repeat, || LirInterpreter::execute(&lir))
            } else if args.jit {
                if cfg!(not(target_arch = "aarch64")) {
                    bail!("The `--jit` feature is currently only supported on ARM64");
                }

                let (duration, func) = run_once(|| Jit::jit(&lir));
                let (func_buff, func) = func?;

                if args.profile {
                    println!("JIT took {:?}", duration);
                }

                let mut cells = [0u8; 30_000];
                let mut buff = [0u8; 30_000];

                let result = run_n(args.repeat, || {
                    func(cells.as_mut_ptr(), buff.as_mut_ptr());
                    Ok(())
                });

                io::stdout().write_all(&buff[0..buff.iter().position(|&b| b == 0).unwrap()])?;

                let _ = func_buff; // Backing memory is now safe to drop

                result
            } else {
                // Should be handled by clap
                unreachable!("pass a backend!");
            }
        }
    };

    if let Ok(duration) = duration && args.profile {
        println!("Execution took: {:?}", duration);
    }

    Ok(())
}

fn run_n<T>(count: u32, mut f: impl FnMut() -> Result<T>) -> Result<Duration> {
    let start = Instant::now();

    for _ in 0..count {
        f()?;
    }

    let end = Instant::now();

    Ok(end - start)
}

fn run_once<R>(f: impl FnOnce() -> R) -> (Duration, R) {
    let start = Instant::now();

    let result = f();

    let end = Instant::now();

    (end - start, result)
}

// // # of cells shown either side of current one
// const CELLS_SHOWN: usize = 5;

// fn repl(mut interpreter: Interpreter) -> Result<()> {
//     let mut line = String::new();

//     loop {
//         print!("> ");
//         io::stdout().flush()?;

//         let mut additional = String::new();
//         io::stdin().read_line(&mut additional)?;

//         if additional.trim() == "q" {
//             println!("Terminating...");
//             break;
//         }

//         line.push_str(&additional);

//         // Need to do a check for unmatched braces so we don't execute a malformed line
//         let line_bytes = line.as_bytes();
//         if line_bytes.iter().filter(|&&b| b == b'[').count()
//             != line_bytes.iter().filter(|&&b| b == b']').count()
//         {
//             println!("(unterminated `[` or `]` - enter next line)");
//             continue;
//         }

//         if let Err(err) = interpreter.execute(line_bytes) {
//             eprintln!("Error: {}", err);
//             eprintln!("Line was discarded");
//         }

//         line.clear();

//         let state = interpreter.state();

//         let mut cell_row = String::new();

//         if state.pos > CELLS_SHOWN {
//             cell_row.push_str("...");
//         }

//         let start_pos = state.pos.saturating_sub(CELLS_SHOWN);

//         for cell_index in start_pos..state.pos + CELLS_SHOWN + 1 {
//             cell_row.push_str(&format!("|{:03}", state.read_cell(cell_index)));
//         }

//         println!("\n{}|...", cell_row);

//         // Each cell is 3 char with a 1-byte prefix
//         let mut cur_cell_pos = (state.pos - start_pos) * 4;

//         if state.pos > CELLS_SHOWN {
//             cur_cell_pos += 3;
//         }

//         println!("{:1$}^^^", "", cur_cell_pos + 1);
//     }

//     Ok(())
// }
