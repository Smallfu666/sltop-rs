# Python Feature Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring the Rust `sltop-rs` TUI up to feature parity with the Python `sltop` v0.3.1

**Architecture:** Incremental enhancement of existing `ui.rs` (ratatui TUI), `app.rs` (state/refresh), and `model.rs` (data types). All changes are in the same interactive session; tightly coupled.

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28, clap 4

**Execution approach:** Inline execution due to tightly coupled UI code. Each phase builds on the previous.

---

### Phase 1: Interactive Navigation

**Files:**
- Modify: `src/ui.rs`

**Goal:** Tab cycling, arrow scroll in all tabs, PageUp/PageDown, consistent scroll state.

- [ ] Add `scroll_offset: usize` to `App` struct and constructor
- [ ] Add `Tab` key to cycle `current_tab` (0→1→2→3→0)
- [ ] Add `KeyCode::Up` scroll for tabs 0, 1, 3 (decrement scroll_offset)
- [ ] Add `KeyCode::Down` scroll for tabs 0, 1, 3 (increment scroll_offset)
- [ ] Add `KeyCode::PageUp` / `PageDown` for fast scroll (±10 lines)
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: add Tab cycling, arrow scroll in all tabs, PageUp/Down"`

### Phase 2: Resources Tab Enhancements

**Files:**
- Modify: `src/ui.rs`

**Goal:** CPU idle bar, QoS label, constraints line, per-partition color palette, memory in GB.

- [ ] Add `PARTITION_COLORS` constant array (10-color palette)
- [ ] Add `fn partition_color(name: &str) -> Color` hash-based lookup
- [ ] CPU idle bar below alloc bar: "CPUs Idle [░░░░░░░░░░] idle/total N%"
- [ ] Add QoS label from matching rule to per-partition card
- [ ] Add constraints line: "MinGPU/job: N  MaxGPU/node: N  Nodes: min–max  MaxCPUs/Node: N"
- [ ] Convert memory display from MB to GB ("1900GB" instead of "1900000MB")
- [ ] Apply `partition_color()` to partition name header
- [ ] Wrap partition cards in a Paragraph with scroll (use `scroll_offset`)
- [ ] Add `total_cpu_idle` to cluster summary
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: resources tab with idle bar, QoS, constraints, colors, GB"`

### Phase 3: Rules Tab Enhancements

**Files:**
- Modify: `src/ui.rs`

**Goal:** AllowAccounts, implied nodes from GPU constraints, total GPUs display.

- [ ] Add AllowAccounts line to rule rendering
- [ ] Add total GPUs per partition from rule.gpu_total
- [ ] Add implied nodes calculation: `ceil(min_gpu / gpu_per_node)`
- [ ] Add TRES breakdown even without GPU entries
- [ ] Apply `partition_color()` to rule partition headers
- [ ] Wrap rules in scrollable Paragraph
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: rules tab with AllowAccounts, implied nodes, total GPUs"`

### Phase 4: Queue Tab Enhancements

**Files:**
- Modify: `src/ui.rs`

**Goal:** Nodelist display in Reason column, partition name colored.

- [ ] Color partition column with `partition_color()`
- [ ] Append nodelist to reason column when available
- [ ] Add Partition column color via styled Span
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: queue tab with nodelist and partition colors"`

### Phase 5: My Jobs Tab — GPU Bar, Connect, Cancel

**Files:**
- Modify: `src/ui.rs`, `src/app.rs`, `src/main.rs`

**Goal:** GPU mini progress bar, Connect to node (srun), Cancel job with confirmation dialog.

- [ ] Add GPU mini-bar to standalone job cards (requested vs partition total)
- [ ] Add `exit_command: Option<String>` to `App` struct
- [ ] Add `KeyCode::Char('c')` to connect to RUNNING job (first selected standalone)
- [ ] Store connect node list in `App` for multi-node selection
- [ ] Add `KeyCode::Char('C')` (shift-c) to cancel selected standalone job
- [ ] Add cancel confirmation state: `confirm_cancel: Option<usize>`
- [ ] Render cancel confirmation overlay when `confirm_cancel` is set
- [ ] Cancel executes: `self.runner.run_scancel(&job_id)`
- [ ] Show cancel result notification
- [ ] Connect exits TUI then runs: `srun --overlap --jobid <ID> --nodelist <NODE> --cpu-bind=none --pty bash`
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: My Jobs with GPU bar, connect, cancel dialog"`

### Phase 6: Polish

**Files:**
- Modify: `src/ui.rs`

**Goal:** Notifications, panel-style borders, idle timeout message, footer with key hints.

- [ ] Add `notification: Option<(String, Instant)>` to App for toast messages
- [ ] Show notification in footer area with timeout (3 seconds)
- [ ] Add idle timeout message with human-friendly duration
- [ ] Improve footer with more key hints
- [ ] Update help text with all new key bindings
- [ ] Build & test: `cargo build` (0 errors), `cargo test` (46 pass)
- [ ] Commit: `git commit -m "feat: notifications, idle message, help text"`

### Phase 7: Final Review

- [ ] Run `cargo build` (0 errors)
- [ ] Run `cargo test` (46 pass)
- [ ] Verify all Python features are covered
- [ ] Commit: `git commit -m "chore: final review and cleanup"`
