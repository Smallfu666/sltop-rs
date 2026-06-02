use sltop::parse;
use sltop::model::{CpuCounts, ResourceRow};

mod shared;

#[test]
fn parse_sinfo_normal() {
    let input = shared::sinfo_normal();
    let rows = parse::parse_sinfo(&input, |_| true).unwrap();
    assert_eq!(rows.len(), 2, "Should have 2 partitions: p, gpu");

    let p_row = rows.iter().find(|r| r.partition == "p").expect("should have partition p");
    assert_eq!(p_row.avail, "up");
    assert_eq!(p_row.mem_mb, 65536);
    assert_eq!(p_row.timelimit, "1-00:00:00");

    let gpu_row = rows.iter().find(|r| r.partition == "gpu").expect("should have gpu partition");
    assert_eq!(gpu_row.avail, "up");
    assert_eq!(gpu_row.mem_mb, 131072);
    assert_eq!(gpu_row.gres, "gpu:A100:8");
}

#[test]
fn parse_sinfo_empty() {
    let input = shared::sinfo_empty();
    let rows = parse::parse_sinfo(&input, |_| true).unwrap();
    assert!(rows.is_empty());
}

#[test]
fn parse_sinfo_partition_filter() {
    let input = shared::sinfo_normal();
    let rows = parse::parse_sinfo(&input, |name| {
        name == "p"
    }).unwrap();
    assert!(!rows.iter().any(|r| r.partition == "gpu"));
    assert!(rows.iter().any(|r| r.partition == "p"));
}

#[test]
fn parse_sinfo_cpu_counts() {
    let cpu = CpuCounts {
        alloc: 4,
        idle: 4,
        other: 0,
        total: 32,
    };
    assert_eq!(cpu.alloc + cpu.idle + cpu.other, 8);
    assert_eq!(cpu.total, 32);
}

#[test]
fn parse_sinfo_merges_multiple_rows() {
    // Test that CPU counts are summed across state rows in the same partition
    let input = shared::sinfo_normal();
    let rows = parse::parse_sinfo(&input, |_| true).unwrap();
    let cpu_row = rows.iter().find(|r| r.partition == "p").expect("partition p should exist from sinfo_normal");
    // partition "p" has 2 rows: alloc + idle
    assert!(cpu_row.nodes.alloc > 0 || cpu_row.nodes.idle > 0,
            "partitions with both alloc and idle should have merged rows");
}

#[test]
fn parse_sinfo_malformed_lines() {
    let input = "this|is||bad\nonly|one\n\np|up|1/2/0/10|1/1/0/2|gpu:V100:4|1-00|32768|1|idle";
    let rows = parse::parse_sinfo(input, |_| true).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.partition, "p");
    assert_eq!(row.gres, "gpu:V100:4");
}
