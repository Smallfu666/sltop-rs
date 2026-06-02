mod cli;
mod model;
mod slurm;
mod parse;
mod app;
mod ui;
mod theme;

use clap::Parser;

fn main() {
    let args = cli::Args::parse();
    let runner = slurm::commands::RealCommandRunner::new();

    let cli_interval = args.interval_secs();
    let partition_filter = args.partition_filter();
    let user_filter = args.user.clone();
    let idle_timeout = args.idle_timeout;

    let state = app::AppState::new(
        cli_interval,
        partition_filter,
        user_filter,
        idle_timeout,
    );

    let app = ui::App::new(state, Box::new(runner));

    match app.run() {
        Ok(Some(cmd)) => {
            eprintln!("Connecting to compute node...");
            let status = std::process::Command::new("sh")
                .args(["-c", &cmd])
                .status();
            if let Err(e) = status {
                eprintln!("Failed to execute: {}", e);
            }
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("TUI error: {}", e);
            std::process::exit(1);
        }
    }
}
