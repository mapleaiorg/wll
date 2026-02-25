use clap::Parser;

mod cli;
mod commands;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();
    commands::run_command(cli)
}
