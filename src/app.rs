use crate::model;
use crate::slurm::commands;
use std::time::Instant;

use anyhow::{Context, Result};

fn parse_duration_secs(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() || s == "N/A" || s == "UNLIMITED" || s == "INFINITE" || s == "Partition_Limit" {
        return u64::MAX;
    }
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let day_parts: Vec<&str> = parts[0].split('-').collect();
            let days: u64 = if day_parts.len() == 2 { day_parts[0].parse().unwrap_or(0) } else { 0 };
            let h: u64 = if day_parts.len() == 2 { day_parts[1].parse().unwrap_or(0) } else { parts[0].parse().unwrap_or(0) };
            let m: u64 = parts[1].parse().unwrap_or(0);
            let s: u64 = parts[2].parse().unwrap_or(0);
            days * 86400 + h * 3600 + m * 60 + s
        }
        2 => {
            let m: u64 = parts[0].parse().unwrap_or(0);
            let s: u64 = parts[1].parse().unwrap_or(0);
            m * 60 + s
        }
        _ => parts[0].parse().unwrap_or(0),
    }
}

pub struct AppState {
    pub cli_interval: u64,
    pub cli_partition_filter: Option<Vec<String>>,
    pub cli_user_filter: Option<String>,
    pub idle_timeout: u64,
    pub last_refresh: Option<Instant>,

    pub resource_rows: Vec<model::ResourceRow>,
    pub rules: Vec<model::Rule>,
    pub qos_limits: Vec<model::QoS>,
    pub cluster_nodes: model::ClusterNodes,
    pub gpu_by_partition: std::collections::HashMap<String, u64>,

    pub queue_jobs: Vec<model::Job>,
    pub sort_col: Option<usize>,
    pub sort_rev: bool,

    pub my_jobs: Vec<model::Job>,
    pub job_groups: model::JobGroups,

    pub running_count: u32,
    pub pending_count: u32,
    pub total_jobs: u32,
}

impl AppState {
    pub fn new(interval: u64, partition_filter: Option<Vec<String>>, user_filter: Option<String>, idle_timeout: u64) -> Self {
        Self {
            cli_interval: interval,
            cli_partition_filter: partition_filter,
            cli_user_filter: user_filter,
            idle_timeout,
            last_refresh: None,
            resource_rows: Vec::new(),
            rules: Vec::new(),
            qos_limits: Vec::new(),
            cluster_nodes: model::ClusterNodes::default(),
            gpu_by_partition: std::collections::HashMap::new(),
            queue_jobs: Vec::new(),
            sort_col: Some(0),
            sort_rev: true,
            my_jobs: Vec::new(),
            job_groups: model::JobGroups::default(),
            running_count: 0,
            pending_count: 0,
            total_jobs: 0,
        }
    }

    pub fn partition_filter_fn(&self) -> Box<dyn Fn(&str) -> bool + '_> {
        match &self.cli_partition_filter {
            Some(filter) => Box::new(move |p: &str| filter.iter().any(|f| f == p)),
            None => Box::new(|_: &str| true),
        }
    }

    pub fn job_filter_fn(&self) -> Box<dyn Fn(&crate::model::Job) -> bool + '_> {
        let part_fn = self.partition_filter_fn();
        match &self.cli_user_filter {
            Some(user) => Box::new(move |j: &crate::model::Job| part_fn(&j.partition) && j.user == *user),
            None => Box::new(move |j: &crate::model::Job| part_fn(&j.partition)),
        }
    }

    pub fn refresh(&mut self, runner: &(impl commands::CommandRunner + ?Sized)) -> Result<()> {
        self.last_refresh = Some(Instant::now());

        let sinfo_str = runner.run_sinfo(&commands::SinfoOpts { format: commands::SINFO_FORMAT.to_string() })
            .context("Failed to run sinfo")?;
        self.resource_rows = crate::parse::parse_sinfo(&sinfo_str, |p| self.partition_filter_fn()(p))
            .context("Failed to parse sinfo")?;

        let cluster_str = runner.run_sinfo(&commands::SinfoOpts { format: "%t|%D".to_string() })
            .unwrap_or_default();
        self.cluster_nodes = crate::parse::parse_cluster_nodes(&cluster_str).unwrap_or_default();

        let queue_str = runner.run_squeue(
            self.cli_user_filter.as_deref(),
            self.cli_partition_filter.as_deref(),
        )?;
        self.queue_jobs = crate::parse::parse_squeue(&queue_str, |j| self.job_filter_fn()(j))?;

        self.gpu_by_partition.clear();
        for job in &self.queue_jobs {
            if job.state != "RUNNING" { continue; }
            let gpu = model::parse_job_gpu(&job.gres);
            if gpu > 0 {
                let nodes: u64 = job.nodes.parse().unwrap_or(1);
                *self.gpu_by_partition.entry(job.partition.clone()).or_default() += gpu * nodes;
            }
        }

        self.apply_sort_to_queue();
        self.update_queue_stats();

        let scontrol_str = runner.run_scontrol_partition()?;
        let sacctmgr_str = runner.run_sacctmgr_qos()?;
        self.qos_limits = crate::parse::parse_sacctmgr_qos(&sacctmgr_str)?;
        self.rules = crate::parse::parse_scontrol_partitions(
            &scontrol_str,
            |p| self.partition_filter_fn()(p),
            &self.qos_limits,
        )?;

        Ok(())
    }

    pub fn update_my_jobs(&mut self) {
        let current_user = std::env::var("USER").unwrap_or_else(|_| std::env::var("LOGNAME").unwrap_or_else(|_| String::from("unknown")));
        self.my_jobs = self.queue_jobs
            .iter()
            .filter(|j| j.user == current_user)
            .cloned()
            .collect();
        self.job_groups = crate::model::group_my_jobs(self.my_jobs.clone());
    }

    pub fn apply_sort_to_queue(&mut self) {
        if let Some(col) = self.sort_col {
            let rev = self.sort_rev;
            self.queue_jobs.sort_by(|a, b| {
                let cmp = match col {
                    0 => a.job_id.parse::<u32>().unwrap_or(0).cmp(&b.job_id.parse::<u32>().unwrap_or(0)),
                    1 => a.partition.cmp(&b.partition),
                    2 => a.user.cmp(&b.user),
                    3 => a.name.cmp(&b.name),
                    4 => a.state.cmp(&b.state),
                    5 => parse_duration_secs(&a.elapsed).cmp(&parse_duration_secs(&b.elapsed)),
                    6 => parse_duration_secs(&a.timelimit).cmp(&parse_duration_secs(&b.timelimit)),
                    7 => a.nodes.parse::<u32>().unwrap_or(0).cmp(&b.nodes.parse::<u32>().unwrap_or(0)),
                    8 => a.gres.cmp(&b.gres),
                    9 => a.reason.cmp(&b.reason),
                    _ => std::cmp::Ordering::Equal,
                };
                if rev { cmp.reverse() } else { cmp }
            });
        }
    }

    fn update_queue_stats(&mut self) {
        self.running_count = 0;
        self.pending_count = 0;
        self.total_jobs = self.queue_jobs.len() as u32;
        for job in &self.queue_jobs {
            match job.state.as_str() {
                "RUNNING" => self.running_count += 1,
                "PENDING" => self.pending_count += 1,
                _ => {}
            }
        }
    }
}
