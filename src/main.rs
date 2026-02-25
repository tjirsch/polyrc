use anyhow::Context;
use clap::Parser as ClapParser;

mod cli;
mod convert;
mod error;
mod formats;
mod ir;
mod parser;
mod writer;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    match args.command {
        cli::Commands::Convert(convert_args) => {
            convert::run(convert_args).context("conversion failed")?;
        }
        cli::Commands::ListFormats => {
            for fmt in formats::Format::all() {
                println!("{:<15} {}", fmt.name(), fmt.description());
            }
        }
    }
    Ok(())
}
