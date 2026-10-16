mod autostart;
mod cli;
mod config;
mod cron;
mod daemon;
mod runner;

use clap::Parser;

fn main() {
    let cli_args = cli::Cli::parse();

    if let Err(err) = cli::handle_cli(cli_args) {
        eprintln!("\nErro: {}", err);
        std::process::exit(1);
    }
}
