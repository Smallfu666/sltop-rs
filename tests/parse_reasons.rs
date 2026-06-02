use sltop::parse;
use sltop::model::Job;

mod shared;

#[test]
fn translate_priority_reason() {
    let meta = parse::translate_reason("Priority", None, None);
    assert!(meta.translation.contains("higher-priority"), "expected priority message");
    assert!(!meta.is_config_error);
}

#[test]
fn translate_qos_min_gres() {
    let mut job = Job {
        job_id: "test".to_string(),
        partition: "gpu".to_string(),
        user: "test".to_string(),
        name: "test".to_string(),
        state: "PENDING".to_string(),
        elapsed: "00:00:00".to_string(),
        timelimit: "1-00:00:00".to_string(),
        nodes: "4".to_string(),
        gres: "gpu:V100:4".to_string(),
        reason: "QOSMinGRES".to_string(),
        dependency: "-".to_string(),
        array_job_id: "-".to_string(),
        array_task_id: "-".to_string(),
        nodelist: "-".to_string(),
    };
    let meta = parse::translate_reason("QOSMinGRES", None, Some(&job));
    assert!(meta.is_config_error);
    assert!(meta.translation.contains("Min GPU"));
}

#[test]
fn translate_req_node_not_avail() {
    let meta = parse::translate_reason("ReqNodeNotAvail,node099", None, None);
    assert!(meta.is_node_unavail);
    assert!(meta.translation.contains("node099"));
}

#[test]
fn translate_partition_down() {
    let meta = parse::translate_reason("PartitionDown", None, None);
    assert!(meta.is_config_error);
    assert!(meta.translation.contains("DOWN"));
}

#[test]
fn translate_none_reason() {
    let meta = parse::translate_reason("N/A", None, None);
    assert!(!meta.is_config_error);
    assert!(!meta.is_node_unavail);
    assert_eq!(meta.translation, "N/A");
}

#[test]
fn translate_empty_reason() {
    let meta = parse::translate_reason("", None, None);
    assert!(!meta.is_config_error);
    assert_eq!(meta.translation, "");
}

#[test]
fn translate_dependency() {
    let meta = parse::translate_reason("Dependency", None, None);
    assert!(meta.translation.contains("dependent"));
    assert!(!meta.is_config_error);
}

#[test]
fn translate_licenses() {
    let meta = parse::translate_reason("Licenses", None, None);
    assert!(meta.translation.contains("license"));
    assert!(!meta.is_config_error);
}

#[test]
fn translate_held_by_admin() {
    let meta = parse::translate_reason("JobHeldAdmin", None, None);
    assert!(meta.is_config_error);
    assert!(meta.translation.contains("admin"));
}

#[test]
fn translate_bad_constraints() {
    let meta = parse::translate_reason("BadConstraints", None, None);
    assert!(meta.is_config_error);
    let lower = meta.translation.to_lowercase();
    assert!(lower.contains("constraints") || lower.contains("no nodes"));
}

#[test]
fn translate_job_held_user() {
    let meta = parse::translate_reason("JobHeldUser", None, None);
    assert!(meta.translation.contains("held"));
    assert!(!meta.is_config_error);
}

#[test]
fn translate_ignores_parens() {
    // Input from squeue has parens: "(QOSMinGRES)"
    let meta = parse::translate_reason("(QOSMinGRES)", None, None);
    assert!(meta.is_config_error);
    assert!(meta.translation.contains("Min GPU"));
}

#[test]
fn translate_invalid_qos() {
    let meta = parse::translate_reason("InvalidQOS", None, None);
    assert!(meta.is_config_error);
    assert!(meta.translation.contains("invalid"));
}

#[test]
fn translate_resources_waiting() {
    let meta = parse::translate_reason("Resources", None, None);
    assert!(meta.translation.contains("CPU|GPU|memory") || meta.translation.contains("memory"));
    assert!(!meta.is_config_error);
}
