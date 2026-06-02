use std::io::{self, Read};
use std::time::{Duration, Instant};

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Row, Table, TableState, Tabs},
    Frame, Terminal,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;

use crate::app::AppState;
use crate::model;
use crate::slurm::commands::CommandRunner;

const C_HI_GREEN: Color = Color::Rgb(0x22, 0xcc, 0x44);
const C_HI_RED: Color = Color::Rgb(0xdd, 0x22, 0x22);
const C_HI_YELLOW: Color = Color::Rgb(0xdd, 0xaa, 0x00);
const C_HI_CYAN: Color = Color::Rgb(0x44, 0xdd, 0xdd);
const C_HI_PURPLE: Color = Color::Rgb(0xdd, 0x88, 0xff);
const C_NODE_ALLOC: Color = Color::Rgb(0xdd, 0x44, 0x22);
const C_NODE_MIX: Color = Color::Rgb(0xdd, 0xaa, 0x00);
const C_NODE_IDLE: Color = Color::Rgb(0x22, 0xcc, 0x44);
const C_NODE_DRAIN: Color = Color::Rgb(0x55, 0x55, 0x55);

const PARTITION_PALETTE: &[Color] = &[
    Color::Rgb(0x44, 0xee, 0x88),
    Color::Rgb(0xdd, 0x88, 0x22),
    Color::Rgb(0x22, 0xaa, 0xdd),
    Color::Rgb(0xdd, 0x44, 0x77),
    Color::Rgb(0x44, 0xdd, 0xdd),
    Color::Rgb(0xdd, 0xdd, 0x44),
    Color::Rgb(0x88, 0x44, 0xdd),
    Color::Rgb(0xdd, 0x66, 0x22),
    Color::Rgb(0x22, 0xdd, 0x88),
    Color::Rgb(0x44, 0x88, 0xdd),
];

fn partition_color(name: &str) -> Color {
    let hash: u64 = name.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    PARTITION_PALETTE[hash as usize % PARTITION_PALETTE.len()]
}

const TAB_NAMES: &[&str] = &[
    " ① Resources ",
    " ② Rules ",
    " ③ Queue ",
    " ④ My Jobs ",
];

const QUEUE_HEADERS: &[&str] = &[
    "JobID", "Partition", "User", "Name", "State", "Elapsed", "Limit", "Nodes", "GRES", "Reason",
];

fn state_style(state: &str) -> Style {
    match state {
        "RUNNING" => Style::default().fg(Color::Green),
        "PENDING" => Style::default().fg(C_HI_YELLOW),
        "COMPLETING" => Style::default().fg(C_HI_CYAN),
        "FAILED" | "CANCELLED" => Style::default().fg(Color::Red),
        "TIMEOUT" => Style::default().fg(C_HI_RED),
        _ => Style::default().fg(Color::White),
    }
}

fn state_symbol(state: &str) -> &'static str {
    match state {
        "RUNNING" => "▶",
        "PENDING" => "⏳",
        "COMPLETING" => "↻",
        "FAILED" | "CANCELLED" => "✗",
        "TIMEOUT" => "⏱",
        _ => " ",
    }
}

fn is_config_error(reason: &str) -> bool {
    let r = reason.trim();
    model::config_error_reasons().iter().any(|re| r.contains(re))
}

fn is_node_unavail(reason: &str) -> bool {
    let r = reason.trim();
    model::node_unavail_reasons().iter().any(|re| r.contains(re))
}

fn reason_span(reason: &str) -> Span<'static> {
    if reason.is_empty() || reason == "(null)" || reason == "None" {
        return Span::raw("");
    }
    if is_config_error(reason) {
        Span::styled(
            format!("⚠ {}", reason),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if is_node_unavail(reason) {
        Span::styled(
            format!("⛔ {}", reason),
            Style::default().fg(C_HI_CYAN).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(reason.to_string())
    }
}

pub struct App {
    pub state: AppState,
    pub running: bool,
    pub current_tab: usize,
    pub show_help: bool,
    pub last_interaction: Instant,
    pub last_auto_refresh: Instant,
    pub table_state: TableState,
    pub scroll_offset: usize,
    pub exit_command: Option<String>,
    pub confirm_cancel: Option<(String, String)>,
    pub notification: Option<(String, Instant)>,
    runner: Box<dyn CommandRunner>,
}

impl App {
    pub fn new(state: AppState, runner: Box<dyn CommandRunner>) -> Self {
        let mut app = Self {
            state,
            running: true,
            current_tab: 0,
            show_help: false,
            last_interaction: Instant::now(),
            last_auto_refresh: Instant::now(),
            table_state: TableState::default(),
            scroll_offset: 0,
            exit_command: None,
            confirm_cancel: None,
            notification: None,
            runner,
        };
        // Initial refresh
        let _ = app.state.refresh(&*app.runner);
        app.state.update_my_jobs();
        app
    }

    fn format_time() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }

    pub fn run(mut self) -> io::Result<Option<String>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let mut stdin = io::stdin();

        let mut last_size = (0u16, 0u16);

        loop {
            // Check terminal size for resize
            if let Ok((w, h)) = crossterm::terminal::size() {
                if (w, h) != last_size {
                    last_size = (w, h);
                    let _ = terminal.autoresize().ok();
                }
            }
            terminal.draw(|frame| self.render(frame))?;

            let now = Instant::now();

            if now - self.last_auto_refresh >= Duration::from_secs(self.state.cli_interval) {
                let _ = self.state.refresh(&*self.runner);
                self.state.update_my_jobs();
                self.last_auto_refresh = now;
            }

            if self.state.idle_timeout > 0
                && now - self.last_interaction > Duration::from_secs(self.state.idle_timeout)
            {
                let timeout = self.state.idle_timeout;
                let msg = if timeout >= 60 { format!("Idle timeout after {}m", timeout / 60) } else { format!("Idle timeout after {}s", timeout) };
                self.notification = Some((msg, Instant::now()));
                break;
            }

            let mut buf = [0u8; 8];
            if stdin.read(&mut buf)? > 0 {
                self.handle_input(&buf[..])?;
            }

            if !self.running {
                break;
            }
        }

        let exit_cmd = self.exit_command.take();
        let notif = self.notification.take();

        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        disable_raw_mode()?;

        if let Some((ref msg, _)) = notif {
            eprintln!("{}", msg);
        }

        Ok(exit_cmd)
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let title = Line::from(Span::styled(
            format!(" SLURM Monitor [v{}] ", env!("CARGO_PKG_VERSION")),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(Paragraph::new(title), chunks[0]);

        let mut status_spans = vec![];
        if let Some(_rt) = self.state.last_refresh {
            let ts = Self::format_time();
            status_spans.push(Span::styled(
                format!(
                    " ✦ run:{}  ⏳ pend:{}  Σ:{}",
                    self.state.running_count,
                    self.state.pending_count,
                    self.state.total_jobs,
                ),
                Style::default().fg(Color::Gray),
            ));
            status_spans.push(Span::raw("  ·  "));
            status_spans.push(Span::styled(ts, Style::default().fg(Color::DarkGray)));
            status_spans.push(Span::raw("  ·  "));
            status_spans.push(Span::styled(
                format!("every {}s", self.state.cli_interval),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            status_spans.push(Span::styled(" idle", Style::default().fg(Color::Gray)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(status_spans)),
            chunks[1],
        );

        let tabs = Tabs::new(
            TAB_NAMES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
        .select(self.current_tab)
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" "));
        frame.render_widget(tabs, chunks[2]);

        match self.current_tab {
            0 => self.render_resources(frame, chunks[3]),
            1 => self.render_rules(frame, chunks[3]),
            2 => self.render_queue(frame, chunks[3]),
            3 => self.render_my_jobs(frame, chunks[3]),
            _ => {}
        }

        let footer = if self.show_help {
            " [1-4] Tab  [Tab] Next  [r] Refresh  [s] Sort  [S] Reverse  [↑↓] Scroll  [c] Connect  [C] Cancel  [h] Hide  [q] Quit"
        } else {
            " sltop — Interactive SLURM Dashboard  |  [h] Help"
        };
        frame.render_widget(
            Paragraph::new(footer).style(Style::default().fg(Color::Gray)),
            chunks[4],
        );

        // Cancel confirmation overlay
        if let Some((ref job_id, ref job_name)) = self.confirm_cancel {
            let overlay_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Length(5),
                    Constraint::Percentage(60),
                ])
                .split(frame.area())[1];
            let overlay_inner = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(30),
                    Constraint::Length(52),
                    Constraint::Percentage(30),
                ])
                .split(overlay_area)[1];
            let lines = vec![
                Line::from(Span::styled(
                    " Cancel Job ",
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw(format!(" Job: {}", job_id))),
                Line::from(Span::raw(format!(" Name: {}", job_name))),
                Line::from(Span::raw("")),
                Line::from(Span::styled(" (y)es  (n)o ", Style::default().fg(C_HI_YELLOW))),
            ];
            let p = Paragraph::new(Text::from(lines))
                .style(Style::default().bg(Color::Rgb(0x44, 0x22, 0x22)));
            frame.render_widget(p, overlay_inner);
        }

        // Notification toast
        if let Some((ref msg, ref since)) = self.notification {
            if since.elapsed() < Duration::from_secs(3) {
                let notif = Paragraph::new(msg.as_str())
                    .style(Style::default().fg(C_HI_CYAN));
                let notif_area = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(msg.len() as u16 + 2)])
                    .split(frame.area())[1];
                frame.render_widget(notif, notif_area);
            } else {
                self.notification = None;
            }
        }
    }

    fn progress_bar(used: u32, total: u32) -> Span<'static> {
        if total == 0 {
            return Span::styled("[          ]", Style::default().fg(Color::DarkGray));
        }
        let width = 10u32;
        let fill = (used * width / total).min(width);
        let bar: String = (0..fill).map(|_| '█').chain((fill..width).map(|_| '░')).collect();
        let pct = used as f64 * 100.0 / total as f64;
        let color = if pct >= 90.0 {
            C_HI_RED
        } else if pct >= 70.0 {
            C_HI_YELLOW
        } else {
            C_HI_GREEN
        };
        Span::styled(format!("[{}]", bar), Style::default().fg(color))
    }

    fn stacked_node_line(&self, label: &str, alloc: u32, mix: u32, idle: u32, drain: u32) -> Line<'static> {
        let total = alloc + mix + idle + drain;
        if total == 0 {
            return Line::from(Span::styled(
                format!("{}  [no nodes]", label),
                Style::default().fg(Color::Gray),
            ));
        }
        let width = 20usize;
        let total_u = total as usize;
        let a = (alloc as usize * width / total_u).min(width);
        let m = (mix as usize * width / total_u).min(width);
        let i = (idle as usize * width / total_u).min(width);
        let d = width.saturating_sub(a + m + i);
        Line::from(vec![
            Span::styled(label.to_string(), Style::default().fg(Color::White)),
            Span::styled("█".repeat(a), Style::default().fg(C_NODE_ALLOC)),
            Span::styled("▓".repeat(m), Style::default().fg(C_NODE_MIX)),
            Span::styled("░".repeat(i), Style::default().fg(C_NODE_IDLE)),
            Span::styled("·".repeat(d), Style::default().fg(C_NODE_DRAIN)),
        ])
    }

    fn find_rule(&self, partition: &str) -> Option<&model::Rule> {
        self.state.rules.iter().find(|r| r.partition == partition)
    }

    fn render_resources(&mut self, frame: &mut Frame, area: Rect) {
        if self.state.resource_rows.is_empty() {
            let p = Paragraph::new("No partition data.").style(Style::default().fg(Color::Gray));
            frame.render_widget(p, area);
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " ── Cluster Summary ──",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        let total_cpu_alloc: u32 = self.state.resource_rows.iter().map(|r| r.cpu.alloc).sum();
        let total_cpu_idle: u32 = self.state.resource_rows.iter().map(|r| r.cpu.idle).sum();
        let total_cpu_all: u32 = self.state.resource_rows.iter().map(|r| r.cpu.total).sum();
        lines.push(Line::from(vec![
            Span::styled(" CPUs     ".to_string(), Style::default().fg(Color::White)),
            Self::progress_bar(total_cpu_alloc, total_cpu_all),
            Span::styled(
                format!(" {} / {}  {:.1}%", total_cpu_alloc, total_cpu_all,
                    if total_cpu_all > 0 { total_cpu_alloc as f64 * 100.0 / total_cpu_all as f64 } else { 0.0 }),
                Style::default().fg(Color::Gray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" CPUs Idle".to_string(), Style::default().fg(Color::White)),
            Self::progress_bar(total_cpu_idle, total_cpu_all),
            Span::styled(
                format!(" {} / {}  {:.1}%", total_cpu_idle, total_cpu_all,
                    if total_cpu_all > 0 { total_cpu_idle as f64 * 100.0 / total_cpu_all as f64 } else { 0.0 }),
                Style::default().fg(Color::Gray),
            ),
        ]));

        let total_gpu_used: u64 = self.state.gpu_by_partition.values().sum();
        let total_gpu_all: u64 = self.state.resource_rows.iter()
            .map(|r| model::gpu_per_node_from_gres(&r.gres) * r.nodes.total() as u64)
            .sum();
        if total_gpu_all > 0 {
            lines.push(Line::from(vec![
                Span::styled(" GPUs     ".to_string(), Style::default().fg(Color::White)),
                Self::progress_bar(total_gpu_used as u32, total_gpu_all as u32),
                Span::styled(
                    format!(" {} / {}  {:.1}%", total_gpu_used, total_gpu_all,
                        total_gpu_used as f64 * 100.0 / total_gpu_all as f64),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }

        let cn = &self.state.cluster_nodes;
        lines.push(self.stacked_node_line("Nodes", cn.alloc, cn.mix, cn.idle, cn.drain));
        lines.push(Line::from(Span::styled(
            format!(
                "  █ alloc:{}  ▓ mix:{}  ░ idle:{}  · drain:{}",
                cn.alloc, cn.mix, cn.idle, cn.drain,
            ),
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::raw("")));

        for row in &self.state.resource_rows {
            let pcolor = partition_color(&row.partition);
            lines.push(Line::from(Span::styled(
                format!(" ── {} ──", row.partition),
                Style::default().fg(pcolor).add_modifier(Modifier::BOLD),
            )));

            let rule = self.find_rule(&row.partition);
            let qos_name = rule.map(|r| r.qos.as_str()).unwrap_or("-");
            let avail_color = if row.avail == "up" { C_HI_GREEN } else { C_HI_RED };
            lines.push(Line::from(vec![
                Span::styled("  ● ", Style::default().fg(avail_color)),
                Span::raw(format!("{}  ", row.avail.to_uppercase())),
                Span::styled("QoS ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(qos_name.to_string(), Style::default().fg(C_HI_CYAN)),
                Span::styled("  MaxTime ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(row.timelimit.clone(), Style::default().fg(C_HI_PURPLE)),
            ]));
            let mem_gb = if row.mem_mb >= 1024 {
                format!("{}GB", row.mem_mb / 1024)
            } else {
                format!("{}MB", row.mem_mb)
            };
            lines.push(Line::from(vec![
                Span::styled("  Mem ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(mem_gb, Style::default().fg(Color::Rgb(0x88, 0xaa, 0xff))),
                Span::styled("  GRES ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(row.gres.clone(), Style::default().fg(C_HI_YELLOW)),
            ]));

            // Constraints line
            if let Some(r) = rule {
                let mut constraints = vec![];
                if r.min_gpu > 0 {
                    constraints.push(format!("MinGPU/job: {}", r.min_gpu));
                }
                if r.max_gpu_node > 0 {
                    constraints.push(format!("MaxGPU/node: {}", r.max_gpu_node));
                }
                if r.min_nodes != "0" && r.min_nodes != "?" {
                    constraints.push(format!("MinNodes: {}", r.min_nodes));
                }
                if r.max_nodes != "UNLIMITED" && r.max_nodes != "?" {
                    constraints.push(format!("MaxNodes: {}", r.max_nodes));
                }
                if r.max_cpus_node != "UNLIMITED" && r.max_cpus_node != "?" {
                    constraints.push(format!("MaxCPUs/Node: {}", r.max_cpus_node));
                }
                if !constraints.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", constraints.join("  ")),
                        Style::default().fg(Color::Rgb(0xbb, 0x99, 0x88)),
                    )));
                }
            }

            lines.push(Line::from(vec![
                Span::styled("  CPUs ".to_string(), Style::default().fg(Color::White)),
                Self::progress_bar(row.cpu.alloc, row.cpu.total),
                Span::styled(
                    format!(" {} / {}  {:.1}%", row.cpu.alloc, row.cpu.total,
                        if row.cpu.total > 0 { row.cpu.alloc as f64 * 100.0 / row.cpu.total as f64 } else { 0.0 }),
                    Style::default().fg(Color::Gray),
                ),
            ]));

            let gpu_total = model::gpu_per_node_from_gres(&row.gres) * row.nodes.total() as u64;
            if gpu_total > 0 {
                let gpu_used = self.state.gpu_by_partition.get(&row.partition).copied().unwrap_or(0);
                lines.push(Line::from(vec![
                    Span::styled("  GPUs ".to_string(), Style::default().fg(Color::White)),
                    Self::progress_bar(gpu_used as u32, gpu_total as u32),
                    Span::styled(
                        format!(" {} / {}  {:.1}%", gpu_used, gpu_total,
                            gpu_used as f64 * 100.0 / gpu_total as f64),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }

            lines.push(self.stacked_node_line(
                "  Nodes",
                row.nodes.alloc,
                row.nodes.mix,
                row.nodes.idle,
                row.nodes.drain,
            ));
            lines.push(Line::from(Span::styled(
                format!(
                    "   █ alloc:{}  ▓ mix:{}  ░ idle:{}  · drain:{}",
                    row.nodes.alloc, row.nodes.mix, row.nodes.idle, row.nodes.drain,
                ),
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(Span::raw("")));
        }

        let max_scroll = lines.len().saturating_sub(area.height as usize);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
        let p = Paragraph::new(Text::from(lines)).scroll((self.scroll_offset as u16, 0));
        frame.render_widget(p, area);
    }

    fn render_rules(&mut self, frame: &mut Frame, area: Rect) {
        if self.state.rules.is_empty() {
            let p = Paragraph::new("No partition data.").style(Style::default().fg(Color::Gray));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::new();
        for rule in &self.state.rules {
            let pcolor = partition_color(&rule.partition);
            lines.push(Line::from(Span::styled(
                format!(" ── {} ──", rule.partition),
                Style::default().fg(pcolor).add_modifier(Modifier::BOLD),
            )));

            let state_color = if rule.state == "UP" { C_HI_GREEN } else { C_HI_RED };
            lines.push(Line::from(vec![
                Span::styled("  ● ", Style::default().fg(state_color)),
                Span::raw(format!("{}  ", rule.state)),
                Span::styled("QoS ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(rule.qos.clone(), Style::default().fg(C_HI_CYAN)),
                Span::styled("  Priority ".to_string(), Style::default().fg(Color::Gray)),
                Span::styled(rule.priority.clone(), Style::default().fg(C_HI_YELLOW)),
                Span::styled("  OverSubscribe ".to_string(), Style::default().fg(Color::Gray)),
                Span::raw(rule.oversubscribe.clone()),
            ]));

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  MaxTime: {}", rule.max_time),
                    Style::default().fg(C_HI_PURPLE),
                ),
                Span::styled(
                    format!("  DefaultTime: {}", rule.default_time),
                    Style::default().fg(Color::Rgb(0xbb, 0x88, 0xff)),
                ),
            ]));

            lines.push(Line::from(Span::raw(format!(
                "  MinNodes: {}  MaxNodes: {}  MaxCPUs/Node: {}",
                rule.min_nodes, rule.max_nodes, rule.max_cpus_node,
            ))));

            // GPU constraints with implied nodes
            if rule.min_gpu > 0 || rule.max_gpu_node > 0 {
                let mut gpu_line = format!("  ⚠ MinGPU/job: {}", rule.min_gpu);
                if rule.max_gpu_node > 0 {
                    gpu_line.push_str(&format!("  MaxGPU/node: {}", rule.max_gpu_node));
                }
                // Implied nodes from GPU constraint
                let gpu_pn = model::gpu_per_node_from_tres(&rule.tres);
                if rule.min_gpu > 0 && gpu_pn > 0 {
                    let implied = (rule.min_gpu + gpu_pn - 1) / gpu_pn;
                    gpu_line.push_str(&format!("  → implies ≥ {} nodes", implied));
                }
                lines.push(Line::from(Span::styled(
                    gpu_line,
                    Style::default()
                        .fg(C_HI_RED)
                        .add_modifier(Modifier::BOLD),
                )));
            }

            // Total GPUs
            if rule.gpu_total > 0 {
                let gpu_pn = model::gpu_per_node_from_tres(&rule.tres);
                lines.push(Line::from(Span::styled(
                    format!("  Total GPUs: {} (≈ {}/node)", rule.gpu_total, gpu_pn),
                    Style::default().fg(C_HI_CYAN),
                )));
            }

            if !rule.allow_groups.is_empty()
                && rule.allow_groups != "ALL"
                && rule.allow_groups != "(null)"
            {
                lines.push(Line::from(Span::styled(
                    format!("  AllowGroups: {}", rule.allow_groups),
                    Style::default().fg(C_HI_GREEN),
                )));
            }

            if !rule.allow_accounts.is_empty()
                && rule.allow_accounts != "ALL"
                && rule.allow_accounts != "(null)"
            {
                lines.push(Line::from(Span::styled(
                    format!("  AllowAccounts: {}", rule.allow_accounts),
                    Style::default().fg(C_HI_CYAN),
                )));
            }

            if !rule.tres.is_empty() && rule.tres != "(null)" {
                lines.push(Line::from(Span::styled(
                    format!("  TRES: {}", rule.tres),
                    Style::default().fg(Color::Rgb(0xaa, 0xbb, 0xcc)),
                )));
            }

            lines.push(Line::from(Span::raw("")));
        }

        let max_scroll = lines.len().saturating_sub(area.height as usize);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
        let p = Paragraph::new(Text::from(lines)).scroll((self.scroll_offset as u16, 0));
        frame.render_widget(p, area);
    }

    fn render_queue(&mut self, frame: &mut Frame, area: Rect) {
        if self.state.queue_jobs.is_empty() {
            let p = Paragraph::new("  No jobs in queue.")
                .style(Style::default().fg(Color::Gray));
            frame.render_widget(p, area);
            return;
        }

        let widths = [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(22),
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(5),
            Constraint::Length(12),
            Constraint::Min(10),
        ];

        let header_cells: Vec<Span> = QUEUE_HEADERS
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let text = if self.state.sort_col == Some(i) {
                    if self.state.sort_rev {
                        format!("{} ▼", h)
                    } else {
                        format!("{} ▲", h)
                    }
                } else {
                    h.to_string()
                };
                Span::styled(
                    text,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect();
        let header = Row::new(header_cells)
            .style(Style::default().bg(Color::Rgb(0x33, 0x33, 0x55)));

        let current_user =
            std::env::var("USER").unwrap_or_else(|_| std::env::var("LOGNAME").unwrap_or_default());
        let rows: Vec<Row> = self
            .state
            .queue_jobs
            .iter()
            .map(|job| {
                let name = if job.name.len() > 22 {
                    format!("{}…", &job.name[..22])
                } else {
                    job.name.clone()
                };
                let pcolor = partition_color(&job.partition);
                let cells = vec![
                    Span::raw(job.job_id.clone()),
                    Span::styled(job.partition.clone(), Style::default().fg(pcolor)),
                    Span::raw(job.user.clone()),
                    Span::raw(name),
                    Span::styled(job.state.clone(), state_style(&job.state)),
                    Span::raw(job.elapsed.clone()),
                    Span::raw(job.timelimit.clone()),
                    Span::raw(job.nodes.clone()),
                    Span::raw(job.gres.clone()),
                    if job.reason.is_empty() || job.reason == "N/A" || job.reason == "(null)" || job.reason == "None" {
                        if !job.nodelist.is_empty() && job.nodelist != "N/A" && job.nodelist != "-" && job.nodelist != "(null)" {
                            Span::styled(format!("nodes: {}", job.nodelist), Style::default().fg(Color::DarkGray))
                        } else {
                            Span::raw("")
                        }
                    } else {
                        reason_span(&job.reason)
                    },
                ];
                let is_me = job.user == current_user;
                Row::new(cells).style(if is_me {
                    Style::default().bg(Color::Rgb(0x33, 0x33, 0x11))
                } else {
                    Style::default()
                })
            })
            .collect();

        let table = Table::new(rows, &widths)
            .header(header)
            .row_highlight_style(
                Style::default()
                    .bg(Color::Rgb(0x33, 0x44, 0x66))
                    .add_modifier(Modifier::BOLD),
            );

        if self.table_state.selected().is_none() && !self.state.queue_jobs.is_empty() {
            self.table_state.select(Some(0));
        }

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_my_jobs(&mut self, frame: &mut Frame, area: Rect) {
        let group = &self.state.job_groups;
        if group.chains.is_empty() && group.arrays.is_empty() && group.standalone.is_empty() {
            let p = Paragraph::new("  No jobs for current user.")
                .style(Style::default().fg(Color::Gray));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::new();

        // Chains
        if !group.chains.is_empty() {
            lines.push(Line::from(Span::styled(
                " ── Chains ──",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for chain in &group.chains {
                let c = model::JobStateCounts::from_jobs(chain);
                lines.push(Line::from(Span::styled(
                    format!(
                        "  Chain: {} jobs  ▶{}  ⏳{}",
                        chain.len(),
                        c.running,
                        c.pending,
                    ),
                    Style::default().fg(C_HI_YELLOW),
                )));
                for (idx, job) in chain.iter().enumerate() {
                    let prefix = if idx == 0 { " ─" } else { " ▼" };
                    let gpu = model::parse_job_gpu(&job.gres);
                    let gpu_s = if gpu > 0 {
                        format!(" gpu:{}", gpu)
                    } else {
                        String::new()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(Color::Gray)),
                        Span::styled(
                            format!(" [{}] {}", job.job_id, job.name),
                            Style::default().fg(Color::White),
                        ),
                        Span::raw(format!(
                            "  {}{}",
                            job.partition, gpu_s,
                        )),
                    ]));
                    if job.state == "PENDING" {
                        let reason = if job.reason.is_empty()
                            || job.reason == "(null)"
                            || job.reason == "None"
                        {
                            String::new()
                        } else {
                            format!(" ({})", job.reason)
                        };
                        lines.push(Line::from(Span::styled(
                            format!("  ⏳ {}{}", job.state, reason),
                            Style::default().fg(C_HI_YELLOW),
                        )));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  ▶ {}/{}", job.elapsed, job.timelimit),
                            Style::default().fg(Color::Green),
                        )));
                    }
                }
                lines.push(Line::from(Span::raw("")));
            }
        }

        // Arrays
        if !group.arrays.is_empty() {
            lines.push(Line::from(Span::styled(
                " ── Arrays ──",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for arr in &group.arrays {
                let first = &arr[0];
                let total = arr.len();
                let running = arr.iter().filter(|j| j.state == "RUNNING").count();
                let pending = arr.iter().filter(|j| j.state == "PENDING").count();
                let completed = arr.iter().filter(|j| j.state == "COMPLETED").count();
                let failed = arr
                    .iter()
                    .filter(|j| {
                        j.state == "FAILED"
                            || j.state == "CANCELLED"
                            || j.state == "TIMEOUT"
                    })
                    .count();
                let done = completed + failed;
                let bar_w = 10;
                let fill = (done * bar_w / total.max(1)).min(bar_w);
                let bar_color = if failed > 0 {
                    C_HI_RED
                } else if done == total {
                    C_HI_GREEN
                } else {
                    C_HI_YELLOW
                };
                let bar: String = (0..bar_w)
                    .map(|i| if i < fill { '█' } else { '░' })
                    .collect();
                lines.push(Line::from(Span::styled(
                    format!("  Array: {} ({})", first.name, first.array_job_id),
                    Style::default().fg(Color::White),
                )));
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  [{}]", bar),
                        Style::default().fg(bar_color),
                    ),
                    Span::styled(
                        format!("  {}/{}", done, total),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
                lines.push(Line::from(Span::styled(
                    format!(
                        "   ▶ {}  ⏳ {}  ✓ {}  ✗ {}",
                        running, pending, completed, failed
                    ),
                    Style::default().fg(Color::Gray),
                )));
                lines.push(Line::from(Span::raw("")));
            }
        }

        // Standalone
        if !group.standalone.is_empty() {
            lines.push(Line::from(Span::styled(
                " ── Standalone ──",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for job in &group.standalone {
                let gpu = model::parse_job_gpu(&job.gres);
                let gpu_s = if gpu > 0 {
                    format!(" gpu:{}", gpu)
                } else {
                    String::new()
                };
                let sym = state_symbol(&job.state);
                let time_str = if job.state == "PENDING" {
                    if !job.reason.is_empty()
                        && job.reason != "(null)"
                        && job.reason != "None"
                    {
                        format!("Reason: {}", job.reason)
                    } else {
                        "Pending".to_string()
                    }
                } else {
                    format!("{}/{}", job.elapsed, job.timelimit)
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} [{}] {}", sym, job.job_id, job.name),
                        state_style(&job.state),
                    ),
                    Span::raw(format!(
                        "  {} {}{}",
                        job.partition,
                        gpu_s,
                        if job.nodes.parse::<u32>().unwrap_or(0) > 0 {
                            format!("  {} node{}", job.nodes,
                                if job.nodes != "1" { "s" } else { "" })
                        } else {
                            String::new()
                        },
                    )),
                ]));
                // GPU mini-bar
                if gpu > 0 {
                    let p_gpu_total = self.state.resource_rows.iter()
                        .find(|r| r.partition == job.partition)
                        .map(|r| model::gpu_per_node_from_gres(&r.gres) * r.nodes.total() as u64)
                        .unwrap_or(0);
                    if p_gpu_total > 0 {
                        let bar_w = 6;
                        let fill = (gpu * bar_w / p_gpu_total).min(bar_w) as usize;
                        let bw = bar_w as usize;
                        let bar: String = (0..fill).map(|_| '█').chain((fill..bw).map(|_| '░')).collect();
                        lines.push(Line::from(Span::styled(
                            format!("   GPU [{}] {}/{}", bar, gpu, p_gpu_total),
                            Style::default().fg(C_HI_CYAN),
                        )));
                    }
                }
                lines.push(Line::from(Span::raw(format!("   {}", time_str))));
                // Action hints
                if job.state == "RUNNING" {
                    lines.push(Line::from(Span::styled(
                        "   [c] Connect  [C] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        "   [C] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(Span::raw("")));
            }
        }

        let max_scroll = lines.len().saturating_sub(area.height as usize);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
        let p = Paragraph::new(Text::from(lines)).scroll((self.scroll_offset as u16, 0));
        frame.render_widget(p, area);
    }

    fn handle_input(&mut self, buf: &[u8]) -> io::Result<()> {
        self.last_interaction = Instant::now();
        let byte = buf[0];
        // Escape sequence: \x1b[A (arrow) or \x1b[5~ (PgUp) or \x1b[6~ (PgDn)
        if byte == 0x1b && buf.len() >= 3 {
            if buf.len() >= 3 && buf[1] == b'[' {
                match buf[2] {
                    b'A' => { // Up
                        match self.current_tab {
                            0 | 1 | 3 => self.scroll_offset = self.scroll_offset.saturating_sub(1),
                            2 => {
                                let i = self.table_state.selected().unwrap_or(0);
                                if i > 0 { self.table_state.select(Some(i - 1)); }
                            }
                            _ => {}
                        }
                        return Ok(());
                    }
                    b'B' => { // Down
                        match self.current_tab {
                            0 | 1 | 3 => self.scroll_offset += 1,
                            2 => {
                                let i = self.table_state.selected().unwrap_or(0);
                                let max = self.state.queue_jobs.len().saturating_sub(1);
                                self.table_state.select(Some((i + 1).min(max)));
                            }
                            _ => {}
                        }
                        return Ok(());
                    }
                    b'5' if buf.len() >= 4 && buf[3] == b'~' => { // PgUp
                        match self.current_tab {
                            0 | 1 | 3 => self.scroll_offset = self.scroll_offset.saturating_sub(10),
                            2 => {
                                let i = self.table_state.selected().unwrap_or(0);
                                self.table_state.select(Some(i.saturating_sub(10)));
                            }
                            _ => {}
                        }
                        return Ok(());
                    }
                    b'6' if buf.len() >= 4 && buf[3] == b'~' => { // PgDn
                        match self.current_tab {
                            0 | 1 | 3 => self.scroll_offset += 10,
                            2 => {
                                let i = self.table_state.selected().unwrap_or(0);
                                let max = self.state.queue_jobs.len().saturating_sub(1);
                                self.table_state.select(Some((i + 10).min(max)));
                            }
                            _ => {}
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        // Single-byte keys
        match byte {
            b'\x1b' => { // Esc
                if self.confirm_cancel.is_some() {
                    self.confirm_cancel = None;
                } else {
                    self.current_tab = 2;
                    self.scroll_offset = 0;
                }
            }
            b'\t' => { // Tab
                self.current_tab = (self.current_tab + 1) % 4;
                self.scroll_offset = 0;
            }
            b'q' => self.running = false,
            b'1' => { self.current_tab = 0; self.scroll_offset = 0; }
            b'2' => { self.current_tab = 1; self.scroll_offset = 0; }
            b'3' => { self.current_tab = 2; self.scroll_offset = 0; }
            b'4' => { self.current_tab = 3; self.scroll_offset = 0; }
            b'r' => {
                let _ = self.state.refresh(&*self.runner);
                self.state.update_my_jobs();
                self.last_auto_refresh = Instant::now();
            }
            b's' => {
                let ncols = QUEUE_HEADERS.len();
                self.state.sort_col = match self.state.sort_col {
                    None => Some(0),
                    Some(c) if c + 1 < ncols => Some(c + 1),
                    _ => None,
                };
                if self.state.sort_col.is_some() {
                    self.state.sort_rev = false;
                }
                self.state.apply_sort_to_queue();
            }
            b'S' => {
                if self.state.sort_col.is_some() {
                    self.state.sort_rev = !self.state.sort_rev;
                    self.state.apply_sort_to_queue();
                }
            }
            b'h' => self.show_help = !self.show_help,
            b'c' => {
                if self.current_tab == 3 {
                    let standalone = &self.state.job_groups.standalone;
                    if !standalone.is_empty() && self.scroll_offset < standalone.len() {
                        if let Some(job) = standalone.get(self.scroll_offset) {
                            if job.state == "RUNNING" && job.nodelist != "-" && job.nodelist != "N/A" && !job.nodelist.is_empty() {
                                let node = job.nodelist.split(',').next().unwrap_or(&job.nodelist);
                                self.exit_command = Some(format!(
                                    "srun --overlap --jobid {} --nodelist {} --cpu-bind=none --pty bash",
                                    job.job_id, node
                                ));
                                self.running = false;
                            }
                        }
                    }
                }
            }
            b'C' => {
                if self.current_tab == 3 && self.confirm_cancel.is_none() {
                    let standalone = &self.state.job_groups.standalone;
                    if !standalone.is_empty() && self.scroll_offset < standalone.len() {
                        if let Some(job) = standalone.get(self.scroll_offset) {
                            self.confirm_cancel = Some((job.job_id.clone(), job.name.clone()));
                        }
                    }
                }
            }
            b'y' | b'Y' => {
                if let Some((job_id, _)) = self.confirm_cancel.take() {
                    let result = self.runner.run_scancel(&job_id);
                    match result {
                        Ok(_) => {
                            self.notification = Some((format!("✓ Cancelled job {}", job_id), Instant::now()));
                            let _ = self.state.refresh(&*self.runner);
                            self.state.update_my_jobs();
                        }
                        Err(e) => {
                            self.notification = Some((format!("✗ Cancel failed: {}", e), Instant::now()));
                        }
                    }
                }
            }
            b'n' | b'N' => self.confirm_cancel = None,
            _ => {}
        }
        Ok(())
    }
}