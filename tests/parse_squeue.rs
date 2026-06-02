use sltop::parse;
use sltop::model::Job;

mod shared;

#[test]
fn parse_squeue_normal() {
    let input = shared::squeue_normal();
    let row_filter = |j: &Job| true;
    let jobs = parse::parse_squeue(&input, row_filter).unwrap();
    assert_eq!(jobs.len(), 8);

    let first = &jobs[0];
    assert_eq!(first.job_id, "1");
    assert_eq!(first.partition, "cpu");
    assert_eq!(first.user, "alice");
    assert_eq!(first.name, "train-bert");
    assert_eq!(first.state, "RUNNING");
    assert_eq!(first.elapsed, "01:23:45");
    assert_eq!(first.timelimit, "2-00:00:00");
    assert_eq!(first.nodes, "4");
    assert_eq!(first.gres, "cpu:8");
    assert_eq!(first.reason, "N/A");
    assert_eq!(first.dependency, "-");
    assert_eq!(first.nodelist, "node001,node002");
}

#[test]
fn parse_squeue_user_filter() {
    let input = shared::squeue_normal();
    let row_filter = |j: &Job| j.user == "alice";
    let jobs = parse::parse_squeue(&input, row_filter).unwrap();
    assert_eq!(jobs.len(), 3);
    for j in &jobs {
        assert_eq!(j.user, "alice");
    }
}

#[test]
fn parse_squeue_empty() {
    let input = shared::squeue_empty();
    let jobs = parse::parse_squeue(&input, |j: &Job| true).unwrap();
    assert!(jobs.is_empty());
}

#[test]
fn parse_squeue_malformed_lines() {
    let input = "too|few|columns\n1|cpu|a|train-bert|RUNNING|01:23:45|2-00:00:00|4|cpu:8|N/A|-|-|-|-\n";
    let jobs = parse::parse_squeue(input, |j: &Job| true).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, "1");
}

#[test]
fn parse_squeue_mixed_state_jobs() {
    let input = shared::squeue_normal();
    let all_jobs: Vec<Job> = parse::parse_squeue(&input, |j: &Job| true).unwrap();
    let running: Vec<&Job> = all_jobs.iter().filter(|j| j.state == "RUNNING").collect();
    let pending: Vec<&Job> = all_jobs.iter().filter(|j| j.state == "PENDING").collect();
    assert_eq!(running.len(), 2);
    assert_eq!(pending.len(), 5);
}

#[test]
fn parse_squeue_reason_codes() {
    let input = shared::squeue_pending_reasons();
    let jobs = parse::parse_squeue(&input, |_| true).unwrap();
    let reasons: Vec<&str> = jobs.iter().map(|j| j.reason.as_str()).collect();
    assert!(reasons.iter().any(|r| *r == "QOSMinGRES"));
    assert!(reasons.iter().any(|r| *r == "InvalidAccount"));
    assert!(reasons.iter().any(|r| *r == "ReqNodeNotAvail,node099"));
}
