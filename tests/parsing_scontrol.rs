use sltop::{model, parse};

mod shared;

#[test]
fn parse_scontrol_partition_normal() {
    let input = shared::scontrol_partition();
    let input2 = shared::sacctmgr_qos();
    let qos = parse::parse_sacctmgr_qos(&input2).unwrap();
    let rules = parse::parse_scontrol_partitions(&input, |_| true, &qos).unwrap();

    assert_eq!(rules.len(), 2, "Should have 2 partitions: gpu, cpu");

    let gpu = rules.iter().find(|r| r.partition == "gpu").expect("gpu partition");
    assert_eq!(gpu.gpu_total, 8);
    assert_eq!(gpu.max_time, "8:00:00");
    assert_eq!(gpu.state, "UP");

    let cpu = rules.iter().find(|r| r.partition == "cpu").expect("cpu partition");
    assert_eq!(cpu.max_time, "01:00:00");
    assert_eq!(cpu.gpu_total, 0);
}

#[test]
fn parse_scontrol_empty() {
    let input = "";
    let qos = Vec::new();
    let rules = parse::parse_scontrol_partitions(input, |_| true, &qos).unwrap();
    assert!(rules.is_empty());
}

#[test]
fn parse_scontrol_partition_filter() {
    let input = shared::scontrol_partition();
    let input2 = shared::sacctmgr_qos();
    let qos = parse::parse_sacctmgr_qos(&input2).unwrap();
    let rules = parse::parse_scontrol_partitions(&input, |name| name == "gpu", &qos).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].partition, "gpu");
}

#[test]
fn parse_sacctmgr_qos_normal() {
    let input = shared::sacctmgr_qos();
    let qos = parse::parse_sacctmgr_qos(&input).unwrap();
    assert_eq!(qos.len(), 3);

    let gpu_high = qos.iter().find(|q| q.name == "gpu_high").unwrap();
    assert_eq!(gpu_high.min_gpu, 4);
    assert_eq!(gpu_high.max_gpu, 8);
    assert_eq!(gpu_high.max_gpu_node, 4);

    let cpu_normal = qos.iter().find(|q| q.name == "cpu_normal").unwrap();
    assert_eq!(cpu_normal.min_gpu, 0);
    assert_eq!(cpu_normal.max_gpu, 0);
}

#[test]
fn gpu_per_node_from_tres() {
    assert_eq!(model::gpu_per_node_from_tres("gres/gpu=480,node=60,mem=15360G"), 8);
    assert_eq!(model::gpu_per_node_from_tres("mem=32G"), 0);
}

#[test]
fn gpu_per_node_from_gres() {
    assert_eq!(model::gpu_per_node_from_gres("gpu:V100:8"), 8);
    assert_eq!(model::gpu_per_node_from_gres("gres/gpu:V100:4"), 4);
    assert_eq!(model::gpu_per_node_from_gres("gres:gpu:8"), 8);
    assert_eq!(model::gpu_per_node_from_gres(""), 0);
}

#[test]
fn parse_dep_ids_basic() {
    assert_eq!(model::parse_dep_ids("afterok:12345:12346"), vec!["12345", "12346"]);
    assert_eq!(model::parse_dep_ids("afterany:1:2:3:4"), vec!["1", "2", "3", "4"]);
    assert_eq!(model::parse_dep_ids(""), Vec::<String>::new());
    assert_eq!(model::parse_dep_ids("(null)"), Vec::<String>::new());
    assert_eq!(model::parse_dep_ids("afterok"), Vec::<String>::new());
}

#[test]
fn parse_job_gpu_zero_cases() {
    assert_eq!(model::parse_job_gpu(""), 0);
    assert_eq!(model::parse_job_gpu("(null)"), 0);
    assert_eq!(model::parse_job_gpu("N/A"), 0);
    assert_eq!(model::parse_job_gpu("-"), 0);
}

#[test]
fn gpu_used_by_partition() {
    let input = "gpu|gres/gpu:V100:8|2\ncpu|gres/gpu:V100:4|4\ngpu|gres/gpu:V100:4|1";
    let result = parse::parse_gpu_used_by_partition(input);
    assert_eq!(result.get("gpu"), Some(&(8 * 2 + 4 * 1))); // 20
    assert_eq!(result.get("cpu"), Some(&(4 * 4)));          // 16
}

#[test]
fn cluster_nodes_empty() {
    let result = parse::parse_cluster_nodes("").unwrap();
    assert_eq!(result, model::ClusterNodes::default());
}

#[test]
fn bucketed_nodes_total() {
    let b = model::BucketedNodes {
        alloc: 10,
        mix: 5,
        idle: 8,
        drain: 2,
    };
    assert_eq!(b.total(), 25);
}

#[test]
fn job_state_counts() {
    let jobs = vec![
        model::Job {
            job_id: "1".to_string(),
            partition: "gpu".to_string(),
            user: "alice".to_string(),
            name: "train".to_string(),
            state: "RUNNING".to_string(),
            elapsed: "01:00".to_string(),
            timelimit: "2-00".to_string(),
            nodes: "4".to_string(),
            gres: "gpu:8".to_string(),
            reason: "N/A".to_string(),
            dependency: "-".to_string(),
            array_job_id: "-".to_string(),
            array_task_id: "-".to_string(),
            nodelist: "node01".to_string(),
        },
        model::Job {
            job_id: "2".to_string(),
            partition: "cpu".to_string(),
            user: "bob".to_string(),
            name: "queue".to_string(),
            state: "PENDING".to_string(),
            elapsed: "00:00".to_string(),
            timelimit: "1-00".to_string(),
            nodes: "2".to_string(),
            gres: "cpu:4".to_string(),
            reason: "Resources".to_string(),
            dependency: "-".to_string(),
            array_job_id: "-".to_string(),
            array_task_id: "-".to_string(),
            nodelist: "-".to_string(),
        },
        model::Job {
            job_id: "3".to_string(),
            partition: "gpu".to_string(),
            user: "charlie".to_string(),
            name: "completed".to_string(),
            state: "COMPLETED".to_string(),
            elapsed: "05:00".to_string(),
            timelimit: "1-00".to_string(),
            nodes: "2".to_string(),
            gres: "gpu:4".to_string(),
            reason: "N/A".to_string(),
            dependency: "-".to_string(),
            array_job_id: "-".to_string(),
            array_task_id: "-".to_string(),
            nodelist: "node05".to_string(),
        },
    ];
    let counts = model::JobStateCounts::from_jobs(&jobs);
    assert_eq!(counts.running, 1);
    assert_eq!(counts.pending, 1);
    assert_eq!(counts.total, 3);
}
