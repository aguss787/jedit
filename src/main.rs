mod app;
mod container;
mod error;

use std::io;

use app::CliApp;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    output: Option<String>,
    input: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let output = args.output.unwrap_or(args.input.clone());
    CliApp::new(args.input, output)?.run()
}
