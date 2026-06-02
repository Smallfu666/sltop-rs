/// Domain types used throughout sltop.

use std::collections::{HashMap, HashSet};

/// Node state from `sinfo %t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeState {
    Alloc,
    Mix,
    Idle,
    Drain,
    Down,
    Other,
}

impl NodeState {
    pub fn from_sinfo(state_raw: &str) -> Self {
        let lowered = state_raw.to_lowercase();
        let s = lowered.trim_end_matches(|c: char| "!+$~#@^*%".contains(c));
        if s.contains("alloc") && s.contains("mix") {
            return NodeState::Mix;
        }
        if s == "mix" {
            return NodeState::Mix;
        }
        if s.contains("alloc") || s.contains("allocated") {
            return NodeState::Alloc;
        }
        if s == "idle" {
            return NodeState::Idle;
        }
        if s == "down" {
            return NodeState::Down;
        }
        // drain, draining, drained, fail, maint, reboot …
        if s.starts_with("drain") {
            return NodeState::Drain;
        }
        NodeState::Other
    }
}

/// CPU counts from `sinfo %C` — format: "alloc/idle/other/total"
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuCounts {
    pub alloc: u32,
    pub idle: u32,
    pub other: u32,
    pub total: u32,
}

/// Node counts from `sinfo %F` — format: "alloc/idle/other/total"
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeCounts {
    pub alloc: u32,
    pub idle: u32,
    pub other: u32,
    pub total: u32,
}

impl NodeCounts {
    pub fn bucketed(&self) -> BucketedNodes {
        BucketedNodes {
            alloc: self.alloc,
            mix: self.other / 2,
            idle: self.idle,
            drain: self.other - self.other / 2,
        }
    }

    /// Convert bucketed node counts back for display
    pub fn from_bucketed(b: BucketedNodes) -> Self {
        Self {
            alloc: b.alloc,
            idle: b.idle,
            other: b.mix + b.drain,
            total: (b.alloc + b.mix + b.idle + b.drain),
        }
    }
}

/// Node counts bucketed into alloc/mix/idle/drain for display.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BucketedNodes {
    pub alloc: u32,
    pub mix: u32,
    pub idle: u32,
    pub drain: u32,
}

impl BucketedNodes {
    pub fn total(&self) -> u32 {
        self.alloc + self.mix + self.idle + self.drain
    }
}

/// A single partition row from `sinfo`.
#[derive(Debug, Clone)]
pub struct ResourceRow {
    pub partition: String,
    pub avail: String,
    pub cpu: CpuCounts,
    pub nodes: BucketedNodes,
    pub gres: String,
    pub timelimit: String,
    pub mem_mb: u64,
}

/// Full partition information from `scontrol show partition`.
#[derive(Debug, Clone)]
pub struct Rule {
    pub partition: String,
    pub state: String,
    pub max_time: String,
    pub default_time: String,
    pub min_nodes: String,
    pub max_nodes: String,
    pub max_cpus_node: String,
    pub priority: String,
    pub preempt_mode: String,
    pub allow_groups: String,
    pub allow_accounts: String,
    pub qos: String,
    pub oversubscribe: String,
    pub tres: String,
    pub gpu_total: u64,
    pub min_gpu: u64,
    pub max_gpu_node: u64,
}

/// QoS limits from `sacctmgr show qos --parsable2`.
#[derive(Debug, Clone)]
pub struct QoS {
    pub name: String,
    pub min_gpu: u64,
    pub max_gpu: u64,
    pub max_gpu_node: u64,
}

/// Parsed job from `squeue`.
#[derive(Debug, Clone)]
pub struct Job {
    pub job_id: String,
    pub partition: String,
    pub user: String,
    pub name: String,
    pub state: String,
    pub elapsed: String,
    pub timelimit: String,
    pub nodes: String,
    pub gres: String,
    pub reason: String,
    pub dependency: String,
    pub array_job_id: String,
    pub array_task_id: String,
    pub nodelist: String,
}

/// Cluster-wide node counts by state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClusterNodes {
    pub alloc: u32,
    pub mix: u32,
    pub idle: u32,
    pub drain: u32,
}

impl ClusterNodes {
    pub fn total(&self) -> u32 {
        self.alloc + self.mix + self.idle + self.drain
    }
}

/// Configuration-error reason codes.
pub fn config_error_reasons() -> &'static [&'static str] {
    &[
        "QOSMinGRES",
        "QOSMaxGRESPerJob",
        "QOSMaxGRESPerNode",
        "QOSMaxWallDurationPerJob",
        "PartitionTimeLimit",
        "PartitionNodeLimit",
        "BadConstraints",
        "InvalidAccount",
        "InvalidQOS",
        "InvalidPartition",
        "DependencyNeverSatisfied",
        "PartitionDown",
        "PartitionInactive",
        "NodeDown",
        "JobHeldAdmin",
        "launch failed requeued held",
    ]
}

/// Node-unavailable reason codes.
pub fn node_unavail_reasons() -> &'static [&'static str] {
    &["ReqNodeNotAvail"]
}

/// GPU per node derived from TRES string like `gres/gpu=1760,node=220`.
pub fn gpu_per_node_from_tres(tres: &str) -> u64 {
    let mut gpu_total = 0u64;
    let mut node_count = 0u64;
    for seg in tres.split(',') {
        let seg = seg.trim();
        if seg.starts_with("gres/gpu=") {
            gpu_total = seg["gres/gpu=".len()..]
                .parse()
                .unwrap_or(0);
        } else if seg.starts_with("node=") {
            node_count = seg["node=".len()..].parse().unwrap_or(0);
        }
    }
    if node_count > 0 && gpu_total > 0 {
        gpu_total / node_count
    } else {
        0
    }
}

/// GPU requested from a GRES string like `gpu:8` or `gres/gpu:8`.
pub fn parse_job_gpu(gres: &str) -> u64 {
    if gres.is_empty()
        || gres == "(null)"
        || gres == "N/A"
        || gres == "-"
        || gres == "(none)"
    {
        return 0;
    }
    for part in gres.split(',') {
        let p = part.trim().to_lowercase();
        let prefix = if p.starts_with("gres/gpu:") { 9 } else if p.starts_with("gres:gpu:") { 9 } else if p.starts_with("gpu:") { 4 } else { continue };
        let val = &p[prefix..];
        if let Ok(v) = val.parse::<u64>() {
            if v > 0 { return v; }
        }
        if let Some(last) = val.rsplit(':').next() {
            if let Ok(v) = last.parse::<u64>() {
                if v > 0 { return v; }
            }
        }
    }
    0
}

/// Derive GPU count from a GPU-per-partition GRES line like `gpu:V100:8`.
pub fn gpu_per_node_from_gres(gres: &str) -> u64 {
    for seg in gres.split(',') {
        let s = seg.trim().to_lowercase();
        let val = if s.starts_with("gres/gpu:") {
            &s[9..]
        } else if s.starts_with("gres:gpu:") {
            &s[9..]
        } else if s.starts_with("gpu:") {
            &s[4..]
        } else {
            continue;
        };
        if let Ok(v) = val.parse::<u64>() {
            if v > 0 { return v; }
        }
        if let Some(last) = val.rsplit(':').next() {
            if let Ok(v) = last.parse::<u64>() {
                if v > 0 { return v; }
            }
        }
    }
    0
}

/// Classify a user's jobs into chains, arrays, and standalone jobs.
#[derive(Debug, Default)]
pub struct JobGroups {
    pub chains: Vec<Vec<Job>>,
    pub arrays: Vec<Vec<Job>>,
    pub standalone: Vec<Job>,
}

/// Extract dependency job IDs from `--dependency` field.
/// Matches afterok:12345, afterany:12345:12346 etc.
pub fn parse_dep_ids(dep_str: &str) -> Vec<String> {
    if dep_str.is_empty() || dep_str == "(null)" || dep_str == "-" {
        return Vec::new();
    }
    let mut ids = Vec::new();
    // Split on ':' and filter for pure-digit parts
    for part in dep_str.split(':') {
        if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
            ids.push(part.to_string());
        }
    }
    ids
}

pub fn group_my_jobs(my_jobs: Vec<Job>) -> JobGroups {
    let mut chains = Vec::new();
    let mut arrays = Vec::new();

    // Build identity maps using owned keys
    let jobs_by_id: HashMap<String, &Job> =
        my_jobs.iter().map(|j| (j.job_id.clone(), j)).collect();
    let all_ids: Vec<String> = my_jobs.iter().map(|j| j.job_id.clone()).collect();

    // Build dependency graph using owned String keys
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();

    for j in &my_jobs {
        let ids = parse_dep_ids(&j.dependency);
        for parent_id in &ids {
            if jobs_by_id.contains_key(parent_id) {
                children.entry(parent_id.clone()).or_default().push(j.job_id.clone());
                parents.entry(j.job_id.clone()).or_default().push(parent_id.clone());
            }
        }
    }

    // Find roots: jobs with children in our set but no parents in our set
    let mut in_chain: HashSet<String> = HashSet::new();
    let mut roots: Vec<String> = all_ids.iter().filter(|jid| {
        children.contains_key(*jid) && !parents.contains_key(*jid)
    }).cloned().collect();
    roots.sort_by_key(|jid| jid.parse::<u64>().unwrap_or(u64::MAX));

    for root in roots {
        let mut chain: Vec<&Job> = Vec::new();
        let mut stack = vec![root.clone()];
        let mut visited: HashSet<String> = HashSet::new();
        while let Some(cur) = stack.pop() {
            if visited.contains(&cur) || !jobs_by_id.contains_key(&cur) {
                continue;
            }
            visited.insert(cur.clone());
            chain.push(jobs_by_id[&cur]);
            in_chain.insert(cur.clone());
            if let Some(child_ids) = children.get(&cur) {
                let mut sorted: Vec<String> = child_ids.clone();
                sorted.sort_by_key(|x| x.parse::<u64>().unwrap_or(u64::MAX));
                for child in sorted.into_iter().rev() {
                    if !visited.contains(&child) {
                        stack.push(child);
                    }
                }
            }
        }
        if chain.len() >= 2 {
            chains.push(chain.into_iter().cloned().collect());
        } else {
            in_chain.remove(&root);
        }
    }

    // Group array jobs
    let mut array_groups: HashMap<String, Vec<&Job>> = HashMap::new();
    for j in &my_jobs {
        if !in_chain.contains(&j.job_id)
            && !j.array_job_id.is_empty()
            && j.array_job_id != "N/A"
            && j.array_job_id != "0"
        {
            array_groups.entry(j.array_job_id.clone()).or_default().push(j);
        }
    }

    let mut sorted_aids: Vec<String> = array_groups.keys().cloned().collect();
    sorted_aids.sort_by_key(|x| x.parse::<u64>().unwrap_or(u64::MAX));

    let mut in_array: HashSet<String> = HashSet::new();
    for aid in &sorted_aids {
        if let Some(group) = array_groups.get(aid) {
            if group.len() >= 2 {
                arrays.push(group.iter().cloned().cloned().collect::<Vec<_>>());
                in_array.insert(aid.clone());
            }
        }
    }

    // Standalone: everything else
    let standalone = my_jobs.into_iter().filter(|j| {
        !in_chain.contains(&j.job_id) && !in_array.contains(&j.job_id)
    }).collect();

    JobGroups {
        chains,
        arrays,
        standalone,
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobStateCounts {
    pub running: u32,
    pub pending: u32,
    pub total: u32,
}

impl JobStateCounts {
    pub fn from_jobs(jobs: &[Job]) -> Self {
        let mut counts = Self::default();
        for job in jobs {
            match job.state.as_str() {
                "RUNNING" => counts.running += 1,
                "PENDING" => counts.pending += 1,
                _ => {}
            };
            counts.total += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dep_ids_basic() {
        assert_eq!(parse_dep_ids("afterok:12345:12346"), vec!["12345", "12346"]);
        assert_eq!(parse_dep_ids("afterany:1:2:3:4"), vec!["1", "2", "3", "4"]);
        assert_eq!(parse_dep_ids(""), Vec::<String>::new());
        assert_eq!(parse_dep_ids("(null)"), Vec::<String>::new());
        assert_eq!(parse_dep_ids("afterok"), Vec::<String>::new());
    }

    #[test]
    fn parse_job_gpu_zero_case() {
        assert_eq!(parse_job_gpu(""), 0);
        assert_eq!(parse_job_gpu("(null)"), 0);
        assert_eq!(parse_job_gpu("N/A"), 0);
        assert_eq!(parse_job_gpu("-"), 0);
    }

    #[test]
    fn test_gpu_per_node_from_tres() {
        assert_eq!(gpu_per_node_from_tres("gres/gpu=1760,node=220"), 8);
        assert_eq!(gpu_per_node_from_tres("mem=32G"), 0);
    }
}
