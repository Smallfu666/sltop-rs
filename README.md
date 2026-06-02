# sltop-rs

**S**LURM dashboa**r**d **T**UI — Rust rewrite of [sltop](https://github.com/whats2000/sltop).

Monitor SLURM cluster partitions, scheduling rules, the full job queue, and your own jobs — all in one terminal window.

## Features

- **4 tabs**: Resources, Rules, Queue, My Jobs — switch with `1` `2` `3` `4`
- **Resources tab**: cluster-wide CPU/GPU/node usage bars, per-partition breakdowns with color-coded utilization
- **Rules tab**: partition scheduling constraints, QoS limits, GPU limits, TRES details
- **Queue tab**: sortable table (10 columns), color-coded job states, config-error detection, current-user highlighting
- **My Jobs tab**: auto-grouped into dependency chains, array jobs (with progress bars), and standalone jobs
- **Auto-refresh**: configurable interval (`-n`, default 10s)
- **Idle timeout**: auto-exit after inactivity (`--idle-timeout`, default 300s, `0` to disable)
- **Reason translation**: SLURM reason codes mapped to human-readable messages (40+ codes)

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1`–`4` | Switch tabs |
| `↑` `↓` | Navigate queue rows |
| `s` | Cycle sort column |
| `S` | Reverse sort direction |
| `r` | Force refresh |
| `Esc` | Jump to Queue tab |
| `h` | Toggle help |
| `q` | Quit |

## Usage

```
sltop [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-n`, `--interval` | `10` | Refresh interval in seconds |
| `-p`, `--partitions` | (all) | Comma-separated partition filter |
| `-u`, `--user` | (all) | Show only USER's jobs |
| `--idle-timeout` | `300` | Exit after N seconds idle (`0` to disable) |

## Requirements

- SLURM client commands: `sinfo`, `squeue`, `scontrol`, `sacctmgr`
- Rust edition 2021+

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

All tests use fixture data — no SLURM cluster required.

## License

MIT — same as the original Python [sltop](https://github.com/whats2000/sltop).
