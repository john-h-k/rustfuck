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
#[command(name = "rustfuck")]
#[command(author = "John Harry Kelly <johnharrykelly@gmail.com>")]
#[command(version = "1.0")]
#[command(
    help_template = "{name}: {about-section}Version: {version}\nWritten by {author-with-newline}\n{usage-heading} {usage}\n{all-args} {tab}"
)]
#[command(about, long_about = None)]
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
