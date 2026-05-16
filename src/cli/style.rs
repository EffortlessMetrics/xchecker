//! Terminal styling helpers for the CLI.

use crossterm::style::{Color, Stylize};
use std::io::IsTerminal;

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
