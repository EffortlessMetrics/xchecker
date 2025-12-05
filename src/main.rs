mod paths;

mod artifact;
mod atomic_write;
mod benchmark;
mod cache;
mod canonicalization;
mod claude;
mod cli;
mod config;
mod doctor;
mod error;
mod error_reporter;
mod exit_codes;
mod fixup;
mod gate;
mod hooks;
mod integration_tests;
mod llm;
mod lock;
mod logging;
mod orchestrator;
mod packet;
mod phase;
mod phases;
mod process_memory;
mod receipt;
mod redaction;
mod ring_buffer;
mod runner;
mod source;
mod spec_id;
mod status;
mod template;
mod tui;
mod types;
mod workspace;
mod wsl;

use error::XCheckerError;

fn main_impl() -> Result<(), XCheckerError> {
    let _matches = cli::build_cli().get_matches();
    cli::run()
}

fn main() {
    if let Err(err) = main_impl() {
        error_reporter::ErrorReporter::report_and_exit(&err);
    }
}
