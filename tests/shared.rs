use std::path::Path;

const FIXTURE_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn load_fixture(path: &str) -> String {
    let full = Path::new(FIXTURE_DIR).join("tests").join("fixtures").join(path);
    std::fs::read_to_string(&full).unwrap_or_default()
}

pub fn sinfo_normal() -> String { load_fixture("sinfo_normal.txt") }
pub fn sinfo_empty() -> String { load_fixture("sinfo_empty.txt") }
pub fn sinfo_empty_partition() -> String { load_fixture("sinfo_empty_partition.txt") }
pub fn squeue_normal() -> String { load_fixture("squeue_normal.txt") }
pub fn squeue_pending_reasons() -> String { load_fixture("squeue_pending_reasons.txt") }
pub fn squeue_empty() -> String { load_fixture("squeue_empty.txt") }
pub fn scontrol_partition() -> String { load_fixture("scontrol_partition.txt") }
pub fn sacctmgr_qos() -> String { load_fixture("sacctmgr_qos.txt") }
