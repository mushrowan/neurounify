mod convert;
mod error;
mod formats;
mod ir;
#[cfg(test)]
mod testdata;

use clap::Parser;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(
    name = "neurounify",
    about = "universal eeg format converter",
    version
)]
struct Cli {
    /// input file (format auto-detected from extension or magic bytes)
    input: PathBuf,

    /// output file (format inferred from extension)
    /// omit to just validate the input (same as --check)
    output: Option<PathBuf>,

    /// validate input without writing (parse + print summary)
    #[arg(short, long)]
    check: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.check || cli.output.is_none() {
        match convert::check(&cli.input) {
            Ok((format, recording)) => {
                convert::print_info(&cli.input, format, &recording);
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
    }

    let output = cli.output.unwrap();
    if let Err(e) = convert::convert(&cli.input, &output) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    eprintln!(
        "{} -> {} done",
        cli.input.display(),
        output.display()
    );
}
