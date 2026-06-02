mod cli;
mod model;
mod slurm;
mod parse;
mod app;
mod ui;

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

    if let Err(e) = app.run() {
        eprintln!("TUI error: {}", e);
        std::process::exit(1);
    }
}
