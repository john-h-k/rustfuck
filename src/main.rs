use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use clap::Parser;

use anyhow::Result;

mod brainfuck;

use brainfuck::Interpreter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The file to execute
    file: PathBuf,
}

fn main() -> Result<()> {
    human_panic::setup_panic!();

    let args = Args::parse();

    let content = fs::read_to_string(args.file)?;

    let interpreter = Interpreter::new(&content);

    interpreter.execute()
}
