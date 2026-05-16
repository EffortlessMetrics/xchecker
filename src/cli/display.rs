//! Terminal display helpers for the CLI.
//!
//! This module owns terminal-specific presentation details (colors and
//! spinners) so command handlers can focus on command behavior.

use crossterm::style::{Color, Stylize};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Check if colored output should be used.
///
/// Returns true only if:
/// - stdout is a terminal (TTY)
/// - NO_COLOR environment variable is not set
fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Return a styled check mark (✓) if colors are enabled, otherwise plain.
pub(super) fn styled_check() -> String {
    if use_color() {
        format!("{}", "✓".with(Color::Green).bold())
    } else {
        "✓".to_string()
    }
}

/// Return a styled warning mark (⚠) if colors are enabled, otherwise plain.
pub(super) fn styled_warning() -> String {
    if use_color() {
        format!("{}", "⚠".with(Color::Yellow).bold())
    } else {
        "⚠".to_string()
    }
}

/// Return styled success text if colors are enabled, otherwise plain.
pub(super) fn styled_success(text: &str) -> String {
    if use_color() {
        format!("{}", text.with(Color::Green).bold())
    } else {
        text.to_string()
    }
}

/// Return styled info text (cyan) if colors are enabled, otherwise plain.
pub(super) fn styled_info(text: &str) -> String {
    if use_color() {
        format!("{}", text.with(Color::Cyan).bold())
    } else {
        text.to_string()
    }
}

pub(super) struct SpinnerGuard {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl SpinnerGuard {
    pub(super) fn new() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Hide cursor to prevent flickering
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide);

        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                print!("\r{} Running health checks...", frames[i]);
                let _ = std::io::stdout().flush();
                i = (i + 1) % frames.len();
                thread::sleep(Duration::from_millis(80));
            }
            // Clear the line when done (use crossterm for portability)
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)
            );
            print!("\r");
            let _ = std::io::stdout().flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }
}

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        // Restore cursor
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
    }
}
