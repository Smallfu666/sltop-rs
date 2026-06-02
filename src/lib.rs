//! sltop — nvtop-inspired interactive SLURM cluster dashboard (Rust rewrite).
//!
//! This crate is the core library of `sltop`. The CLI binary is in `src/main.rs`.

pub mod app;
pub mod cli;
pub mod model;
pub mod parse;
pub mod slurm;

// Re-export key types for convenience
pub use model::{
    BucketedNodes, CpuCounts, ClusterNodes, Job, JobGroups, node_unavail_reasons,
    config_error_reasons, gpu_per_node_from_gres, gpu_per_node_from_tres,
    parse_dep_ids, parse_job_gpu, JobStateCounts,
};
pub use parse::{
    parse_sinfo, parse_squeue, parse_scontrol_partitions, parse_sacctmgr_qos,
    parse_cluster_nodes, parse_gpu_used_by_partition, translate_reason, ReasonMeta,
    ParseError,
};
pub use slurm::commands::{
    CommandRunner, CommandOutput, SinfoOpts, RealCommandRunner, FakeCommandRunner,
    SINFO_FORMAT, JOB_FORMAT,
};
