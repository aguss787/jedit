mod app;
mod container;
mod error;

#[cfg(test)]
mod fixtures;

use std::io;

use app::CliApp;
use clap::Parser;

/// View and edit JSON file
#[derive(Debug, Parser)]
struct Args {
    /// Output file to write to. Defaults to overwrite the input file
    #[arg(short, long)]
    output: Option<String>,
    /// JSON file to edit
    input: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let output = args.output.unwrap_or(args.input.clone());
    let app = Box::leak(Box::new(CliApp::new(args.input, output)?));
    app.run()
}
