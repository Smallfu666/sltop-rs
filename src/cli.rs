use clap::{CommandFactory, Parser};

/// sltop — nvtop-inspired interactive SLURM cluster dashboard.
///
/// Monitor partitions, scheduling rules, the full job queue,
/// and your own running/pending jobs from a single terminal window.
#[derive(Parser, Debug)]
#[command(name = "sltop", version, about, long_about = None)]
pub struct Args {
    /// Refresh interval in seconds (default: 10)
    #[arg(short = 'n', long = "interval")]
    pub interval: Option<u64>,

    /// Comma-separated partition filter (default: all)
    #[arg(short = 'p', long = "partitions")]
    pub partitions: Option<String>,

    /// Show only jobs for USER in the Queue tab (default: all)
    #[arg(short = 'u', long = "user")]
    pub user: Option<String>,

    /// Exit after SECS seconds of no interaction (default: 300, 0 to disable)
    #[arg(long = "idle-timeout", default_value = "300")]
    pub idle_timeout: u64,
}

impl Args {
    pub fn interval_secs(&self) -> u64 {
        self.interval.unwrap_or(10)
    }

    pub fn partition_filter(&self) -> Option<Vec<String>> {
        self.partitions.as_ref().map(|p| {
            p.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    }
}
