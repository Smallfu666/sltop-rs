/// Trait for executing SLURM commands.
/// RealCommandRunner executes via std::process::Command.
/// FakeCommandRunner returns fixture data for tests.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<CommandOutput>;

    fn run_sinfo(&self, opts: &SinfoOpts) -> anyhow::Result<String> {
        let mut args = Vec::new();
        args.push("--noheader");
        if !opts.format.is_empty() {
            args.push("-o");
            args.push(&opts.format);
        }
        let out = self.run("sinfo", &args)?;
        if out.exit_code != 0 {
            anyhow::bail!(
                "sinfo failed (exit {}): {}",
                out.exit_code,
                out.stderr
            );
        }
        Ok(out.stdout)
    }

    fn run_squeue(&self, user: Option<&str>, partition_filter: Option<&[String]>) -> anyhow::Result<String> {
        let mut cmd: Vec<String> = vec!["--noheader".to_string(), "-o".to_string(), JOB_FORMAT.to_string()];
        if let Some(u) = user {
            cmd.push("-u".to_string());
            cmd.push(u.to_string());
        }
        if let Some(pf) = partition_filter {
            let part_str = pf.join(",");
            cmd.push("--partition".to_string());
            cmd.push(part_str);
        }
        let cmd_refs: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
        let out = self.run("squeue", &cmd_refs)?;
        if out.exit_code != 0 {
            anyhow::bail!(
                "squeue failed (exit {}): {}",
                out.exit_code,
                out.stderr
            );
        }
        Ok(out.stdout)
    }

    fn run_scontrol_partition(&self) -> anyhow::Result<String> {
        let out = self.run("scontrol", &["show", "partition"])?;
        if out.exit_code != 0 {
            anyhow::bail!(
                "scontrol failed (exit {}): {}",
                out.exit_code,
                out.stderr
            );
        }
        Ok(out.stdout)
    }

    fn run_sacctmgr_qos(&self) -> anyhow::Result<String> {
        let args: Vec<&str> = vec![
            "show", "qos",
            "--noheader", "--parsable2",
            "format=Name,MinTRES,MaxTRES,MaxTRESPerNode",
        ];
        let out = self.run("sacctmgr", &args)?;
        if out.exit_code != 0 {
            // Graceful degradation: return empty instead of crashing.
            return Ok(String::new());
        }
        Ok(out.stdout)
    }

    fn run_scontrol_hostnames(&self, nodelist: &str) -> anyhow::Result<String> {
        let out = self.run("scontrol", &["show", "hostnames", nodelist])?;
        if out.exit_code != 0 {
            return Ok(String::new());
        }
        Ok(out.stdout)
    }

    fn run_scancel(&self, job_id: &str) -> anyhow::Result<CommandOutput> {
        self.run("scancel", &[job_id])
    }

    fn run_sacctmgr(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = self.run("sacctmgr", args)?;
        if out.exit_code != 0 {
            return Ok(String::new());
        }
        Ok(out.stdout)
    }
}

/// Output from a SLURM command.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Option bundle for `sinfo` calls.
#[derive(Debug, Clone, Default)]
pub struct SinfoOpts {
    pub format: String,
}

pub const SINFO_FORMAT: &str = "%P|%a|%C|%F|%G|%l|%m|%D|%t";
pub const JOB_FORMAT: &str = "%i|%P|%u|%j|%T|%M|%l|%D|%b|%R|%E|%F|%K|%N";

/// Real implementation — executes SLURM commands via std::process::Command.
pub struct RealCommandRunner;

impl RealCommandRunner {
    pub fn new() -> Self {
        Self
    }
}

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<CommandOutput> {
        use std::process::{Command, Stdio};
        let output = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| anyhow::anyhow!("{program} {:?}: {}", args, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(CommandOutput {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

/// Fake implementation — returns fixture data for tests.
pub struct FakeCommandRunner {
    pub fixtures: std::sync::Mutex<Fixtures>,
}

impl Default for FakeCommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeCommandRunner {
    pub fn new() -> Self {
        Self {
            fixtures: std::sync::Mutex::new(Fixtures::default()),
        }
    }

    pub fn set_sinfo(&self, data: String) {
        self.fixtures.lock().unwrap().sinfo = data;
    }

    pub fn set_squeue(&self, data: String) {
        self.fixtures.lock().unwrap().squeue = data;
    }

    pub fn set_scontrol(&self, data: String) {
        self.fixtures.lock().unwrap().scontrol = data;
    }

    pub fn set_sacctmgr(&self, data: String) {
        self.fixtures.lock().unwrap().sacctmgr = data;
    }
}

/// Fixture data storage for FakeCommandRunner.
#[derive(Debug, Default)]
pub(crate) struct Fixtures {
    sinfo: String,
    squeue: String,
    scontrol: String,
    sacctmgr: String,
}

impl CommandRunner for FakeCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<CommandOutput> {
        let fixtures = self.fixtures.lock().unwrap();
        let output = match program {
            "sinfo" => {
                let stdout = fixtures.sinfo.clone();
                CommandOutput {
                    stdout,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "squeue" => {
                let stdout = fixtures.squeue.clone();
                CommandOutput {
                    stdout,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "scontrol" => {
                if args.len() >= 2 && args[0] == "show" && args[1] == "hostnames" {
                    return Ok(CommandOutput {
                        stdout: String::new(),
                        stderr: String::new(),
                        exit_code: 0,
                    });
                }
                let stdout = fixtures.scontrol.clone();
                CommandOutput {
                    stdout,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            "sacctmgr" => {
                let stdout = fixtures.sacctmgr.clone();
                CommandOutput {
                    stdout,
                    stderr: String::new(),
                    exit_code: 0,
                }
            }
            _ => {
                return Ok(CommandOutput {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 0,
                });
            }
        };
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_runner_returns_empty_successfully() {
        let runner = FakeCommandRunner::new();
        let out = runner.run_sinfo(&SinfoOpts::default()).unwrap();
        assert!(out.is_empty());
    }
}
