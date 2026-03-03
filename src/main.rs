use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "neurounify", about = "universal eeg format converter")]
struct Cli {
    /// input file path
    #[arg(short, long)]
    input: PathBuf,

    /// output file path
    #[arg(short, long)]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    println!(
        "converting {} -> {}",
        cli.input.display(),
        cli.output.display()
    );
}
