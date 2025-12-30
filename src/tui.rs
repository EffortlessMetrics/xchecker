//! Terminal User Interface for xchecker workspace management
//!
//! This module provides an interactive TUI for browsing workspace specs,
//! viewing receipt summaries, and monitoring spec status.
//!
//! Requirements:
//! - 4.4.1: Display specs list with tags and last status
//! - 4.4.2: Keyboard-only navigation (arrow keys, j/k, Enter, q)
//! - 4.4.3: Read-only in V16 (no destructive operations)

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::io;
use std::path::Path;

use crate::receipt::ReceiptManager;
use crate::workspace::Workspace;

/// TUI application state
pub struct TuiApp {
    /// Workspace being displayed
    workspace: Workspace,
    /// Path to the workspace file (stored for potential future use)
    #[allow(dead_code)]
    workspace_path: std::path::PathBuf,
    /// List of spec statuses
    spec_statuses: Vec<SpecStatus>,
    /// Currently selected spec index
    selected_index: usize,
    /// List state for the specs list
    list_state: ListState,
    /// Whether to show detailed view for selected spec
    show_details: bool,
    /// Summary statistics
    summary: WorkspaceSummary,
}

/// Status information for a spec
#[derive(Clone)]
pub struct SpecStatus {
    pub id: String,
    pub tags: Vec<String>,
    pub status: String,
    pub latest_phase: Option<String>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub pending_fixups: u32,
    pub has_errors: bool,
    pub receipt_summary: Option<ReceiptSummary>,
}

/// Summary of a receipt for display
#[derive(Clone)]
pub struct ReceiptSummary {
    pub phase: String,
    pub exit_code: i32,
    pub emitted_at: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub runner: String,
    pub warnings_count: usize,
    pub outputs_count: usize,
}

/// Workspace summary statistics
#[derive(Default)]
pub struct WorkspaceSummary {
    pub total_specs: u32,
    pub successful_specs: u32,
    pub failed_specs: u32,
    pub pending_specs: u32,
    pub not_started_specs: u32,
    pub stale_specs: u32,
    pub total_pending_fixups: u32,
    pub total_errors: u32,
}

impl TuiApp {
    /// Create a new TUI application from a workspace path
    pub fn new(workspace_path: &Path) -> Result<Self> {
        let workspace = Workspace::load(workspace_path)?;
        let spec_statuses = Self::collect_spec_statuses(&workspace);
        let summary = Self::calculate_summary(&spec_statuses);

        let mut list_state = ListState::default();
        if !spec_statuses.is_empty() {
            list_state.select(Some(0));
        }

        Ok(Self {
            workspace,
            workspace_path: workspace_path.to_path_buf(),
            spec_statuses,
            selected_index: 0,
            list_state,
            show_details: false,
            summary,
        })
    }

    /// Collect status information for all specs in the workspace
    fn collect_spec_statuses(workspace: &Workspace) -> Vec<SpecStatus> {
        let stale_threshold = chrono::Duration::days(7);
        let now = chrono::Utc::now();

        workspace
            .list_specs()
            .iter()
            .map(|spec| {
                let base_path = crate::paths::spec_root(&spec.id);
                let receipt_manager = ReceiptManager::new(&base_path);
                let receipts = receipt_manager.list_receipts().unwrap_or_default();

                let (status, latest_phase, last_activity, has_errors, receipt_summary) = if receipts
                    .is_empty()
                {
                    ("not_started".to_string(), None, None, false, None)
                } else {
                    let latest = receipts.last().unwrap();
                    let last_activity_time = latest.emitted_at;
                    let is_stale = now.signed_duration_since(last_activity_time) > stale_threshold;

                    let summary = ReceiptSummary {
                        phase: latest.phase.clone(),
                        exit_code: latest.exit_code,
                        emitted_at: latest.emitted_at,
                        model: latest.model_full_name.clone(),
                        runner: latest.runner.clone(),
                        warnings_count: latest.warnings.len(),
                        outputs_count: latest.outputs.len(),
                    };

                    if latest.exit_code == 0 {
                        let all_phases_complete = receipts
                            .iter()
                            .any(|r| r.phase == "final" && r.exit_code == 0);
                        if all_phases_complete {
                            (
                                if is_stale { "stale" } else { "success" }.to_string(),
                                Some(latest.phase.clone()),
                                Some(last_activity_time),
                                false,
                                Some(summary),
                            )
                        } else {
                            (
                                if is_stale { "stale" } else { "pending" }.to_string(),
                                Some(latest.phase.clone()),
                                Some(last_activity_time),
                                false,
                                Some(summary),
                            )
                        }
                    } else {
                        (
                            "failed".to_string(),
                            Some(latest.phase.clone()),
                            Some(last_activity_time),
                            true,
                            Some(summary),
                        )
                    }
                };

                let pending_fixups = count_pending_fixups_for_spec(&spec.id);

                SpecStatus {
                    id: spec.id.clone(),
                    tags: spec.tags.clone(),
                    status,
                    latest_phase,
                    last_activity,
                    pending_fixups,
                    has_errors,
                    receipt_summary,
                }
            })
            .collect()
    }

    /// Calculate summary statistics from spec statuses
    fn calculate_summary(spec_statuses: &[SpecStatus]) -> WorkspaceSummary {
        let mut summary = WorkspaceSummary {
            total_specs: spec_statuses.len() as u32,
            ..Default::default()
        };

        for spec in spec_statuses {
            match spec.status.as_str() {
                "success" => summary.successful_specs += 1,
                "failed" => summary.failed_specs += 1,
                "pending" => summary.pending_specs += 1,
                "not_started" => summary.not_started_specs += 1,
                "stale" => summary.stale_specs += 1,
                _ => {}
            }
            summary.total_pending_fixups += spec.pending_fixups;
            if spec.has_errors {
                summary.total_errors += 1;
            }
        }

        summary
    }

    /// Move selection up
    fn select_previous(&mut self) {
        if self.spec_statuses.is_empty() {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.spec_statuses.len() - 1;
        }
        self.list_state.select(Some(self.selected_index));
    }

    /// Move selection down
    fn select_next(&mut self) {
        if self.spec_statuses.is_empty() {
            return;
        }
        if self.selected_index < self.spec_statuses.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
        self.list_state.select(Some(self.selected_index));
    }

    /// Toggle details view
    fn toggle_details(&mut self) {
        if !self.spec_statuses.is_empty() {
            self.show_details = !self.show_details;
        }
    }

    /// Get the currently selected spec status
    fn selected_spec(&self) -> Option<&SpecStatus> {
        self.spec_statuses.get(self.selected_index)
    }
}

/// Count pending fixups for a spec (reused from cli.rs)
fn count_pending_fixups_for_spec(spec_id: &str) -> u32 {
    use crate::fixup::{FixupMode, FixupParser};

    let base_path = crate::paths::spec_root(spec_id);
    let review_md_path = base_path.join("artifacts").join("30-review.md");

    if !review_md_path.exists() {
        return 0;
    }

    let review_content = match std::fs::read_to_string(&review_md_path) {
        Ok(content) => content,
        Err(_) => return 0,
    };

    let fixup_parser = match FixupParser::new(FixupMode::Preview, base_path.into()) {
        Ok(parser) => parser,
        Err(_) => return 0,
    };

    if !fixup_parser.has_fixup_markers(&review_content) {
        return 0;
    }

    match fixup_parser.parse_diffs(&review_content) {
        Ok(diffs) => diffs.len() as u32,
        Err(_) => 0,
    }
}

/// Run the TUI application
pub fn run_tui(workspace_path: &Path) -> Result<()> {
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Create app state
    let mut app = TuiApp::new(workspace_path)?;

    // Run the main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

/// Main application loop
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut TuiApp,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => {
                    if !app.show_details {
                        app.select_previous();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !app.show_details {
                        app.select_next();
                    }
                }
                KeyCode::Enter => app.toggle_details(),
                KeyCode::Esc => {
                    if app.show_details {
                        app.show_details = false;
                    } else {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Render the UI
fn ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(5), // Summary
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer/help
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_summary(f, app, chunks[1]);

    if app.show_details {
        render_details(f, app, chunks[2]);
    } else {
        render_specs_list(f, app, chunks[2]);
    }

    render_footer(f, app, chunks[3]);
}

/// Render the header
fn render_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "xchecker ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("workspace: "),
        Span::styled(&app.workspace.name, Style::default().fg(Color::Yellow)),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Workspace TUI "),
    );
    f.render_widget(header, area);
}

/// Render the summary section
fn render_summary(f: &mut Frame, app: &TuiApp, area: Rect) {
    let summary = &app.summary;

    let summary_text = vec![
        Line::from(vec![
            Span::raw("Total: "),
            Span::styled(
                format!("{}", summary.total_specs),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::raw(format!("{}", summary.successful_specs)),
            Span::raw("  "),
            Span::styled("✗ ", Style::default().fg(Color::Red)),
            Span::raw(format!("{}", summary.failed_specs)),
            Span::raw("  "),
            Span::styled("⋯ ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}", summary.pending_specs)),
            Span::raw("  "),
            Span::styled("○ ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", summary.not_started_specs)),
        ]),
        Line::from(vec![
            Span::raw("Stale: "),
            Span::styled(
                format!("{}", summary.stale_specs),
                if summary.stale_specs > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::raw("  Pending fixups: "),
            Span::styled(
                format!("{}", summary.total_pending_fixups),
                if summary.total_pending_fixups > 0 {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::raw("  Errors: "),
            Span::styled(
                format!("{}", summary.total_errors),
                if summary.total_errors > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]),
    ];

    let summary_widget = Paragraph::new(summary_text)
        .block(Block::default().borders(Borders::ALL).title(" Summary "));
    f.render_widget(summary_widget, area);
}

/// Render the specs list
fn render_specs_list(f: &mut Frame, app: &TuiApp, area: Rect) {
    let items: Vec<ListItem> = app
        .spec_statuses
        .iter()
        .map(|spec| {
            let status_style = match spec.status.as_str() {
                "success" => Style::default().fg(Color::Green),
                "failed" => Style::default().fg(Color::Red),
                "pending" => Style::default().fg(Color::Yellow),
                "stale" => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM),
                "not_started" => Style::default().fg(Color::DarkGray),
                _ => Style::default(),
            };

            let status_icon = match spec.status.as_str() {
                "success" => "✓",
                "failed" => "✗",
                "pending" => "⋯",
                "stale" => "⏰",
                "not_started" => "○",
                _ => "?",
            };

            let tags_str = if spec.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", spec.tags.join(", "))
            };

            let phase_str = spec
                .latest_phase
                .as_ref()
                .map(|p| format!(" @ {}", p))
                .unwrap_or_default();

            let fixups_str = if spec.pending_fixups > 0 {
                format!(" ({} fixups)", spec.pending_fixups)
            } else {
                String::new()
            };

            let line = Line::from(vec![
                Span::styled(format!("{} ", status_icon), status_style),
                Span::styled(&spec.id, Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(phase_str, Style::default().fg(Color::Cyan)),
                Span::styled(tags_str, Style::default().fg(Color::Blue)),
                Span::styled(fixups_str, Style::default().fg(Color::Magenta)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Specs "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.list_state.clone());
}

/// Render the details view for the selected spec
fn render_details(f: &mut Frame, app: &TuiApp, area: Rect) {
    let spec = match app.selected_spec() {
        Some(s) => s,
        None => {
            let empty = Paragraph::new("No spec selected")
                .block(Block::default().borders(Borders::ALL).title(" Details "));
            f.render_widget(empty, area);
            return;
        }
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Spec ID: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&spec.id),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                &spec.status,
                match spec.status.as_str() {
                    "success" => Style::default().fg(Color::Green),
                    "failed" => Style::default().fg(Color::Red),
                    "pending" => Style::default().fg(Color::Yellow),
                    "stale" => Style::default().fg(Color::Yellow),
                    _ => Style::default().fg(Color::DarkGray),
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Tags: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(if spec.tags.is_empty() {
                "(none)".to_string()
            } else {
                spec.tags.join(", ")
            }),
        ]),
        Line::from(vec![
            Span::styled(
                "Latest Phase: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(spec.latest_phase.as_deref().unwrap_or("-")),
        ]),
        Line::from(vec![
            Span::styled(
                "Last Activity: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                spec.last_activity
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "Pending Fixups: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", spec.pending_fixups),
                if spec.pending_fixups > 0 {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default()
                },
            ),
        ]),
        Line::from(""),
    ];

    // Add receipt summary if available
    if let Some(receipt) = &spec.receipt_summary {
        lines.push(Line::from(Span::styled(
            "Latest Receipt:",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(vec![
            Span::raw("  Phase: "),
            Span::raw(&receipt.phase),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Exit Code: "),
            Span::styled(
                format!("{}", receipt.exit_code),
                if receipt.exit_code == 0 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                },
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Emitted: "),
            Span::raw(
                receipt
                    .emitted_at
                    .format("%Y-%m-%d %H:%M:%S UTC")
                    .to_string(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Model: "),
            Span::raw(&receipt.model),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Runner: "),
            Span::raw(&receipt.runner),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Outputs: "),
            Span::raw(format!("{}", receipt.outputs_count)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  Warnings: "),
            Span::styled(
                format!("{}", receipt.warnings_count),
                if receipt.warnings_count > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "No receipts available",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let details = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Details: {} ", spec.id)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(details, area);
}

/// Render the footer with help text
fn render_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let help_text = if app.show_details {
        "Esc: Back  q: Quit"
    } else {
        "↑/k: Up  ↓/j: Down  Enter: Details  q: Quit"
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_workspace() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join("workspace.yaml");

        let mut workspace = Workspace::new("test-project");
        workspace
            .add_spec("spec-1", vec!["backend".to_string()], false)
            .unwrap();
        workspace
            .add_spec(
                "spec-2",
                vec!["frontend".to_string(), "ui".to_string()],
                false,
            )
            .unwrap();
        workspace.save(&workspace_path).unwrap();

        (temp_dir, workspace_path)
    }

    #[test]
    fn test_tui_app_creation() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let app = TuiApp::new(&workspace_path).unwrap();

        assert_eq!(app.workspace.name, "test-project");
        assert_eq!(app.spec_statuses.len(), 2);
        assert_eq!(app.selected_index, 0);
        assert!(!app.show_details);
    }

    #[test]
    fn test_tui_app_navigation() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let mut app = TuiApp::new(&workspace_path).unwrap();

        // Initial selection
        assert_eq!(app.selected_index, 0);

        // Move down
        app.select_next();
        assert_eq!(app.selected_index, 1);

        // Move down again (should wrap to 0)
        app.select_next();
        assert_eq!(app.selected_index, 0);

        // Move up (should wrap to last)
        app.select_previous();
        assert_eq!(app.selected_index, 1);

        // Move up
        app.select_previous();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_tui_app_details_toggle() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let mut app = TuiApp::new(&workspace_path).unwrap();

        assert!(!app.show_details);

        app.toggle_details();
        assert!(app.show_details);

        app.toggle_details();
        assert!(!app.show_details);
    }

    #[test]
    fn test_tui_app_empty_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join("workspace.yaml");

        let workspace = Workspace::new("empty-project");
        workspace.save(&workspace_path).unwrap();

        let app = TuiApp::new(&workspace_path).unwrap();

        assert_eq!(app.workspace.name, "empty-project");
        assert!(app.spec_statuses.is_empty());
        assert_eq!(app.summary.total_specs, 0);
    }

    #[test]
    fn test_tui_app_summary_calculation() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let app = TuiApp::new(&workspace_path).unwrap();

        // Both specs should be "not_started" since there are no receipts
        assert_eq!(app.summary.total_specs, 2);
        assert_eq!(app.summary.not_started_specs, 2);
        assert_eq!(app.summary.successful_specs, 0);
        assert_eq!(app.summary.failed_specs, 0);
        assert_eq!(app.summary.pending_specs, 0);
    }

    #[test]
    fn test_spec_status_fields() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let app = TuiApp::new(&workspace_path).unwrap();

        let spec1 = &app.spec_statuses[0];
        assert_eq!(spec1.id, "spec-1");
        assert_eq!(spec1.tags, vec!["backend"]);
        assert_eq!(spec1.status, "not_started");
        assert!(spec1.latest_phase.is_none());
        assert!(spec1.last_activity.is_none());
        assert_eq!(spec1.pending_fixups, 0);
        assert!(!spec1.has_errors);

        let spec2 = &app.spec_statuses[1];
        assert_eq!(spec2.id, "spec-2");
        assert_eq!(spec2.tags, vec!["frontend", "ui"]);
    }

    #[test]
    fn test_selected_spec() {
        let (_temp_dir, workspace_path) = create_test_workspace();

        let mut app = TuiApp::new(&workspace_path).unwrap();

        let selected = app.selected_spec().unwrap();
        assert_eq!(selected.id, "spec-1");

        app.select_next();
        let selected = app.selected_spec().unwrap();
        assert_eq!(selected.id, "spec-2");
    }

    #[test]
    fn test_navigation_with_empty_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join("workspace.yaml");

        let workspace = Workspace::new("empty-project");
        workspace.save(&workspace_path).unwrap();

        let mut app = TuiApp::new(&workspace_path).unwrap();

        // Navigation should be no-op with empty workspace
        app.select_next();
        assert_eq!(app.selected_index, 0);

        app.select_previous();
        assert_eq!(app.selected_index, 0);

        // Toggle details should be no-op
        app.toggle_details();
        assert!(!app.show_details);

        // Selected spec should be None
        assert!(app.selected_spec().is_none());
    }
}
