/// Parsers for SLURM CLI command outputs.
/// All parsers are pure functions with no I/O.

use crate::model::{
    ClusterNodes, Job, NodeState, QoS, ResourceRow, Rule,
    gpu_per_node_from_gres, gpu_per_node_from_tres,
    parse_job_gpu,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("empty input")]
    EmptyInput,
    #[error("invalid format: {0}")]
    InvalidFormat(String),
}

// ── sinfo parser ──────────────────────────────────────────────────────────────
// Format: %P|%a|%C|%F|%G|%l|%m|%D|%t
//   %P     partition name
//   %a     availability (UP/DOWN)
//   %C     CPU A/I/O/T (alloc/idle/other/total)
//   %F     Node A/I/O/T (alloc/idle/other/total)
//   %G     GRES per node (e.g., gpu:V100:8)
//   %l     time limit
//   %m     memory per node in MB
//   %D     total nodes in that state
//   %t     node state (alloc, mix, idle, drain, down)

pub fn parse_sinfo(input: &str, filter_fn: impl Fn(&str) -> bool) -> Result<Vec<ResourceRow>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows_by_partition: HashMap<String, ResourceRow> = HashMap::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Remove trailing '*' from partition name (indicates DOWN)
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 9 {
            continue;
        }

        let partition_raw = parts[0];
        let partition = partition_raw.trim_end_matches('*');
        if !filter_fn(partition) {
            continue;
        }

        let avail_str = if partition_raw.ends_with('*') { "DOWN" } else { "UP" };

        let cpu = parse_cpu_counts(parts[2]).unwrap_or_default();
        let node_counts: crate::model::NodeCounts = parse_node_counts(parts[3]).unwrap_or_default();
        let gres = parts[4].trim().to_string();
        let timelimit = parts[5].trim().to_string();
        let mem_mb = parts[6].trim().parse::<u64>().unwrap_or(0);

        let bucketed = node_counts.bucketed();

        rows_by_partition
            .entry(partition.to_string())
            .and_modify(|row: &mut ResourceRow| {
                row.cpu.alloc = row.cpu.alloc.saturating_add(cpu.alloc);
                row.cpu.idle = row.cpu.idle.saturating_add(cpu.idle);
                row.cpu.other = row.cpu.other.saturating_add(cpu.other);
                row.cpu.total = row.cpu.total.saturating_add(cpu.total);
                // Update avail to UP if any row shows UP
                if avail_str == "UP" {
                    row.avail = String::from("up");
                }
                row.nodes.alloc = row.nodes.alloc.saturating_add(bucketed.alloc);
                row.nodes.mix = row.nodes.mix.saturating_add(bucketed.mix);
                row.nodes.idle = row.nodes.idle.saturating_add(bucketed.idle);
                row.nodes.drain = row.nodes.drain.saturating_add(bucketed.drain);
            })
            .or_insert_with(|| ResourceRow {
                partition: partition.to_string(),
                avail: avail_str.to_string(),
                cpu,
                nodes: bucketed,
                gres,
                timelimit,
                mem_mb,
            });
    }

    let mut result: Vec<ResourceRow> = rows_by_partition.into_values().collect();
    result.sort_by(|a, b| a.partition.cmp(&b.partition));
    Ok(result)
}

fn parse_cpu_counts(s: &str) -> Option<crate::model::CpuCounts> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 4 {
        return None;
    }
    Some(crate::model::CpuCounts {
        alloc: parts[0].parse().ok()?,
        idle: parts[1].parse().ok()?,
        other: parts[2].parse().ok()?,
        total: parts[3].parse().ok()?,
    })
}

fn parse_node_counts(s: &str) -> Option<crate::model::NodeCounts> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 4 {
        return None;
    }
    Some(crate::model::NodeCounts {
        alloc: parts[0].parse().ok()?,
        idle: parts[1].parse().ok()?,
        other: parts[2].parse().ok()?,
        total: parts[3].parse().ok()?,
    })
}

// ── squeue parser ─────────────────────────────────────────────────────────────
// Format: %i|%P|%u|%j|%T|%M|%l|%D|%b|%R|%E|%F|%K|%N
//   %i     job id
//   %P     partition
//   %u     user
//   %j     job name
//   %T     state
//   %M     elapsed time
//   %l     time limit
//   %D     number of nodes
//   %b     GRES (e.g., gpu:8)
//   %R     reason
//   %E     dependency
//   %F     array_job_id
//   %K     array_task_id
//   %N     nodelist

pub fn parse_squeue(input: &str, filter_fn: impl Fn(&Job) -> bool) -> Result<Vec<Job>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut jobs: Vec<Job> = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 14 {
            continue;
        }
        let job = Job {
            job_id: parts[0].trim().to_string(),
            partition: parts[1].trim().to_string(),
            user: parts[2].trim().to_string(),
            name: parts[3].trim().to_string(),
            state: parts[4].trim().to_string(),
            elapsed: parts[5].trim().to_string(),
            timelimit: parts[6].trim().to_string(),
            nodes: parts[7].trim().to_string(),
            gres: parts[8].trim().to_string(),
            reason: parts[9].trim().to_string(),
            dependency: parts[10].trim().to_string(),
            array_job_id: parts[11].trim().to_string(),
            array_task_id: parts[12].trim().to_string(),
            nodelist: parts[13].trim().to_string(),
        };
        if filter_fn(&job) {
            jobs.push(job);
        }
    }
    Ok(jobs)
}

// ── scontrol partition parser ─────────────────────────────────────────────────
// Parses block-format output from `scontrol show partition`.
// Blocks separated by blank lines; fields are spread across lines.

pub fn parse_scontrol_partitions(
    input: &str,
    filter_fn: impl Fn(&str) -> bool,
    qos_limits: &[QoS],
) -> Result<Vec<Rule>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut rules: Vec<Rule> = Vec::new();
    // Split on blank lines to get per-partition blocks
    let blocks: Vec<&str> = input
        .split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Build QoS lookup
    let qos_map: HashMap<&str, &QoS> = qos_limits.iter().map(|q| (q.name.as_str(), q)).collect();

    for block in blocks {
        let mut fields: HashMap<String, String> = HashMap::new();
        // Join all lines and tokenize on whitespace
        for token in block.split_whitespace() {
            if let Some((key, val)) = token.split_once('=') {
                fields.insert(key.to_string(), val.to_string());
            }
        }

        let partition = fields.get("PartitionName").cloned().unwrap_or_default();
        if partition.is_empty() || !filter_fn(&partition) {
            continue;
        }

        let gpu_total = fields.get("TRES")
            .map(|tres| gpu_per_node_from_tres(tres))
            .unwrap_or(0);

        let qos_name = fields.get("QoS").cloned().unwrap_or_else(|| "-".to_string());
        let qos_limit_default = QoS {
            name: String::new(),
            min_gpu: 0,
            max_gpu: 0,
            max_gpu_node: 0,
        };
        let qos_limit_ref = qos_map.get(qos_name.as_str()).copied().unwrap_or(&qos_limit_default);

        rules.push(Rule {
            partition,
            state: fields.get("State").cloned().unwrap_or_else(|| "?".to_string()),
            max_time: fields.get("MaxTime").cloned().unwrap_or_else(|| "?".to_string()),
            default_time: fields.get("DefaultTime").cloned().unwrap_or_else(|| "?".to_string()),
            min_nodes: fields.get("MinNodes").cloned().unwrap_or_else(|| "0".to_string()),
            max_nodes: fields.get("MaxNodes").cloned().unwrap_or_else(|| "UNLIMITED".to_string()),
            max_cpus_node: fields.get("MaxCPUsPerNode").cloned().unwrap_or_else(|| "UNLIMITED".to_string()),
            priority: fields.get("PriorityTier").cloned().unwrap_or_else(|| "?".to_string()),
            preempt_mode: fields.get("PreemptMode").cloned().unwrap_or_else(|| "?".to_string()),
            allow_groups: fields.get("AllowGroups").cloned().unwrap_or_else(|| "ALL".to_string()),
            allow_accounts: fields.get("AllowAccounts").cloned().unwrap_or_else(|| "ALL".to_string()),
            qos: qos_name,
            oversubscribe: fields.get("OverSubscribe").cloned().unwrap_or_else(|| "?".to_string()),
            tres: fields.get("TRES").cloned().unwrap_or_default(),
            gpu_total,
            min_gpu: qos_limit_ref.min_gpu,
            max_gpu_node: qos_limit_ref.max_gpu_node,
        });
    }

    rules.sort_by(|a, b| a.partition.cmp(&b.partition));
    Ok(rules)
}

// ── sacctmgr QoS parser ───────────────────────────────────────────────────────
// Format from `--parsable2`: Name|MinTRES|MaxTRES|MaxTRESPerNode

pub fn parse_sacctmgr_qos(input: &str) -> Result<Vec<QoS>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut result: Vec<QoS> = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.is_empty() || parts[0].trim().is_empty() {
            continue;
        }

        let name = parts[0].trim().to_string();
        let min_tres = if parts.len() > 1 { parts[1].trim() } else { "" };
        let max_tres = if parts.len() > 2 { parts[2].trim() } else { "" };
        let max_node = if parts.len() > 3 { parts[3].trim() } else { "" };

        result.push(QoS {
            name,
            min_gpu: parse_tres_gpu(min_tres),
            max_gpu: parse_tres_gpu(max_tres),
            max_gpu_node: parse_tres_gpu(max_node),
        });
    }

    Ok(result)
}

fn parse_tres_gpu(s: &str) -> u64 {
    for seg in s.split(',') {
        let seg = seg.trim();
        // Try gres/gpu=X or gres:gpu:X patterns
        if seg.starts_with("gres/gpu=") {
            if let Ok(v) = seg[9..].split(':').next().unwrap_or("").parse::<u64>() {
                if v > 0 {
                    return v;
                }
            }
        } else if seg.starts_with("gres:gpu=") {
            if let Ok(v) = seg[9..].split(':').next().unwrap_or("").parse::<u64>() {
                if v > 0 {
                    return v;
                }
            }
        } else if seg.starts_with("gres/gpu:") {
            if let Ok(v) = seg[9..].split(':').next().unwrap_or("").parse::<u64>() {
                if v > 0 {
                    return v;
                }
            }
        }
    }
    0
}

#[derive(Debug, Clone, Default)]
pub struct ReasonMeta {
    pub is_config_error: bool,
    pub is_node_unavail: bool,
    pub translation: String,
}

/// Translate a SLURM reason code into human-readable text.
/// The `rule` and `job` params let us insert actual limits into messages.
pub fn translate_reason(reason: &str, rule: Option<&Rule>, job: Option<&Job>) -> ReasonMeta {
    let r = reason.trim();
    if r.is_empty() || r == "-" || r == "None" {
        return ReasonMeta {
            is_config_error: false,
            is_node_unavail: false,
            translation: r.to_string(),
        };
    }

    // squeue wraps codes in parens: "(QOSMinGRES)" → "QOSMinGRES"
    let display_reason = if r.starts_with('(') && r.ends_with(')') {
        &r[1..r.len() - 1]
    } else {
        r
    };

    // Strip comma-separated node suffix: "ReqNodeNotAvail,node001" → "ReqNodeNotAvail"
    let base = display_reason.split(',').next().unwrap_or(display_reason).trim();
    let suffix = display_reason.get(base.len()..).unwrap_or("");
    let suffix = suffix.strip_prefix(',').unwrap_or("").trim();

    let base_lower = base.to_lowercase();

    // Extract actual values
    let min_gpu = rule.map(|r| r.min_gpu).unwrap_or(0);
    let max_gpu_node = rule.map(|r| r.max_gpu_node).unwrap_or(0);
    let max_time = rule.map(|r| r.max_time.as_str()).unwrap_or("?");
    let max_nodes = rule.map(|r| r.max_nodes.as_str()).unwrap_or("?");
    let min_nodes = rule.map(|r| r.min_nodes.as_str()).unwrap_or("?");

    let req_gpu = job.map(|j| gpu_per_node_from_gres(&j.gres)).unwrap_or(0);
    let req_nodes = job.and_then(|j| j.nodes.parse::<u64>().ok()).unwrap_or(0);
    let req_timelimit = job.map(|j| j.timelimit.as_str()).unwrap_or("?");

    fn gpu_detail(limit: u64, actual: u64, label: &str) -> String {
        if actual > 0 && limit > 0 {
            format!("{label} (you requested {actual}, limit is {limit})")
        } else if limit > 0 {
            format!("{label} (limit is {limit})")
        } else {
            label.to_string()
        }
    }

    let map: HashMap<&str, &str> = [
        ("Priority", "Waiting — higher-priority jobs are ahead in the queue"),
        ("Resources", "Waiting — not enough CPU/GPU/memory free right now"),
        ("WaitingForScheduling", "Just submitted — scheduler hasn't processed it yet"),
        ("BeginTime", "Waiting — job has a future start time (--begin)"),
        ("Dependency", "Waiting for dependent job(s) to finish"),
        ("DependencyNeverSatisfied", "⚠ Dependency can never be satisfied (check --dependency)"),
        ("QOSMaxGRESPerUser", "⚠ Your total GPU usage exceeds QoS user limit"),
        ("QOSMaxJobsPerUser", "⚠ Too many running jobs for your QoS"),
        ("QOSMaxSubmitJobPerUser", "⚠ Too many pending+running jobs for your QoS"),
        ("QOSMaxCpuPerUser", "⚠ Your total CPU usage exceeds QoS user limit"),
        ("QOSMaxNodePerUser", "⚠ Your total node usage exceeds QoS user limit"),
        ("QOSMaxMemoryPerUser", "⚠ Your total memory usage exceeds QoS user limit"),
        ("QOSJobLimit", "⚠ QoS total concurrent job limit reached"),
        ("QOSResourceLimit", "⚠ QoS total resource limit reached"),
        ("QOSUsageThreshold", "⚠ QoS usage threshold exceeded"),
        ("PartitionDown", "⚠ Partition is DOWN"),
        ("PartitionInactive", "⚠ Partition is INACTIVE"),
        ("PartitionTimeLimit", "⚠ Walltime exceeds partition limit (max {max_time})"),
        ("NodeDown", "⚠ Required node(s) are DOWN"),
        ("BadConstraints", "⚠ No nodes match your constraint/feature request"),
        ("InactiveLimit", "⚠ Job exceeded inactive time limit"),
        ("InvalidAccount", "⚠ Account not valid or not permitted in this partition"),
        ("InvalidQOS", "⚠ QoS is invalid for this account/partition"),
        ("InvalidPartition", "⚠ Partition does not exist or you lack permission"),
        ("AssocMaxJobsLimit", "⚠ Account/association running job limit reached"),
        ("AssocGrpCpuLimit", "⚠ CPU quota exhausted for your account/group"),
        ("AssocGrpNodeLimit", "⚠ Node quota exhausted for your account/group"),
        ("AssocGrpSubmitJobsLimit", "⚠ Submit quota exhausted for your account/group"),
        ("Licenses", "Waiting for software license(s)"),
        ("JobHeldUser", "Job held by user (scontrol release to release)"),
        ("JobHeldAdmin", "⚠ Job held by admin — contact sysadmin"),
        ("launch failed requeued held", "⚠ Launch failed — job requeued and held"),
    ]
    .into_iter()
    .collect();

    let mut is_config_error = false;
    let mut is_node_unavail = false;
    let mut message = display_reason.to_string();

    if let Some(pattern) = map.get(base) {
        message = pattern.to_string();
    } else if base == "QOSMinGRES" {
        message = gpu_detail(min_gpu, req_gpu, "⚠ Min GPU not met — partition requires ≥{min_gpu} GPU/job");
    } else if base == "QOSMaxGRESPerJob" {
        message = gpu_detail(max_gpu_node, req_gpu, "⚠ GPU request exceeds QoS per-job maximum");
    } else if base == "QOSMaxGRESPerNode" {
        if req_gpu > 0 && req_nodes > 0 && max_gpu_node > 0 {
            message = format!("⚠ GPUs/node exceeds QoS limit (you requested {req_gpu} GPU across {req_nodes} node(s), max {max_gpu_node}/node)");
        } else {
            message = format!("⚠ GPUs/node exceeds QoS limit (max {max_gpu_node}/node)");
        }
    } else if base == "QOSMaxWallDurationPerJob" {
        if req_timelimit != "?" && req_timelimit != "N/A" && max_time != "?" {
            message = format!("⚠ Walltime exceeds QoS limit (you requested {req_timelimit}, max {max_time})");
        } else {
            message = format!("⚠ Walltime exceeds QoS limit (max {max_time})");
        }
    } else if base == "PartitionNodeLimit" {
        if req_nodes > 0 {
            message = format!("⚠ Node count outside partition limits (you requested {req_nodes}, allowed: {min_nodes}–{max_nodes})");
        } else {
            message = format!("⚠ Node count outside partition limits (allowed: {min_nodes}–{max_nodes})");
        }
    } else if base == "ReqNodeNotAvail" {
        if !suffix.is_empty() {
            message = format!("⚠ Requested specific node(s) not available: {suffix}");
        } else {
            message = "⚠ Requested specific node(s) not available".to_string();
        }
    } else if base == "DependencyNeverSatisfied" {
        message = "⚠ Dependency can never be satisfied (check --dependency)".to_string();
    } else {
        // Fuzzy fallback
        if base_lower.contains("qosmin") {
            message = format!("⚠ Min resource requirement not met for QoS{}", if min_gpu > 0 { format!(" (partition min GPU: {min_gpu})") } else { String::new() });
        } else if base_lower.contains("qosmax") {
            message = "⚠ Exceeds a QoS maximum limit".to_string();
        } else if base_lower.contains("assoc") {
            message = "⚠ Account/association limit reached".to_string();
        } else if base_lower.contains("partition") {
            message = "⚠ Partition constraint violated".to_string();
        } else if base_lower.contains("dependency") {
            message = "Waiting on job dependency".to_string();
        } else if base_lower.contains("held") {
            message = "Job is held".to_string();
        }
    }

    // Check config error and node unavail flags
    let creasons = crate::model::config_error_reasons();
    if base == "PartitionTimeLimit" || base == "PartitionNodeLimit" || base == "QOSMaxWallDurationPerJob" {
        is_config_error = true;
    } else if creasons.contains(&base) {
        is_config_error = true;
    }

    let nunavail = crate::model::node_unavail_reasons();
    if nunavail.contains(&base) {
        is_node_unavail = true;
    }

    ReasonMeta {
        is_config_error,
        is_node_unavail,
        translation: message,
    }
}

// ── sinfo cluster-wide node states ────────────────────────────────────────────
// Format: %t|%D  (state|node_count)

pub fn parse_cluster_nodes(input: &str) -> Result<ClusterNodes, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(ClusterNodes::default());
    }

    let mut nodes = ClusterNodes::default();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 2 {
            continue;
        }
        let state_raw = parts[0].trim();
        let count = parts[1].trim().parse::<u32>().unwrap_or(0);

        match NodeState::from_sinfo(state_raw) {
            NodeState::Alloc => nodes.alloc += count,
            NodeState::Mix => nodes.mix += count,
            NodeState::Idle => nodes.idle += count,
            NodeState::Drain | NodeState::Down => nodes.drain += count,
            _ => {}
        }
    }

    Ok(nodes)
}

// ── sinfo GPU used by partition ───────────────────────────────────────────────
// Uses squeue -t RUNNING -o "%P|%b|%D"

pub fn parse_gpu_used_by_partition(input: &str) -> HashMap<String, u64> {
    let mut result: HashMap<String, u64> = HashMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 3 {
            continue;
        }
        let partition = parts[0].trim().to_string();
        let tres_node = parts[1].trim();
        let nodes_str = parts[2].trim();
        let nodes = nodes_str.parse::<u64>().unwrap_or(0);

        let gpu_per_node = parse_job_gpu(tres_node);
        *result.entry(partition).or_default() += gpu_per_node * nodes;
    }
    result
}
