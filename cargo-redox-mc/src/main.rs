use cargo_redox_mc::cli::{normalize_args, Cli};
use clap::Parser;

fn main() {
    let args = normalize_args(std::env::args().collect());
    let cli = Cli::parse_from(args);

    if let Err(error) = cargo_redox_mc::commands::dispatch(cli) {
        eprintln!("[redox-mc] error: {error}");
        std::process::exit(1);
    }
}
