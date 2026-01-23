//! Command-line interface for xchecker
//!
//! This module provides the CLI commands and argument parsing for the
//! xchecker tool, starting with basic spec generation functionality.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

// Stable public API imports from crate root
// _Requirements: FR-CLI-2_
use crate::{CliArgs, Config, ExitCode, OrchestratorHandle, PhaseId, XCheckerError, emit_jcs};

// Internal module imports (not part of stable public API)
use crate::atomic_write::write_file_atomic;
use crate::error::{ConfigError, PhaseError};
use crate::error_reporter::{ErrorReport, utils as error_utils};
use crate::logging::Logger;
use crate::orchestrator::OrchestratorConfig;
use crate::redaction::SecretRedactor;
use crate::source::SourceResolver;
use crate::spec_id::sanitize_spec_id;

/// xchecker - Claude orchestration tool for spec generation
#[derive(Parser)]
#[command(name = "xchecker")]
#[command(about = "A CLI tool for orchestrating spec generation workflows using LLM providers")]
#[command(long_about = r#"
xchecker is a deterministic, token-efficient pipeline that transforms rough ideas
into detailed implementation plans through a structured phase-based approach.

EXAMPLES:
  # Generate a spec from stdin input
  echo "Create a REST API for user management" | xchecker spec user-api

  # Generate a spec from a GitHub issue
  xchecker spec issue-123 --source gh --gh owner/repo

  # Generate a spec from local filesystem context
  xchecker spec my-feature --source fs --repo /path/to/project

  # Run in dry-run mode to see what would be executed
  xchecker spec test-spec --dry-run --verbose

  # Check status of a spec
  xchecker status user-api

  # Resume from a specific phase
  xchecker resume user-api --phase design

  # Clean up spec artifacts
  xchecker clean user-api

  # Run performance benchmarks
  xchecker benchmark --file-count 50 --iterations 3

  # Run integration tests
  xchecker test --components --smoke

CONFIGURATION:
  Configuration is loaded with precedence: CLI flags > config file > defaults
  Config file is discovered by searching upward from CWD for .xchecker/config.toml
  Use --config to specify an explicit config file path

PHASES:
  Requirements → Design → Tasks → Review → Fixup → Final
  Each phase produces artifacts and receipts for auditability
  Use --dry-run to see planned execution without making LLM calls

For more information, see: https://github.com/your-org/xchecker
"#)]
#[command(version)]
pub struct Cli {
    /// Path to configuration file (overrides discovery)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Model to use for LLM provider calls
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Maximum number of turns for Claude interactions
    #[arg(long, global = true)]
    pub max_turns: Option<u32>,

    /// Maximum bytes for packet content
    #[arg(long, global = true)]
    pub packet_max_bytes: Option<usize>,

    /// Maximum lines for packet content
    #[arg(long, global = true)]
    pub packet_max_lines: Option<usize>,

    /// Output format for Claude CLI (stream-json or text)
    #[arg(long, global = true)]
    pub output_format: Option<String>,

    /// Runner mode: native (direct), wsl (Windows only), or auto (detect best option)
    #[arg(long, global = true)]
    pub runner_mode: Option<String>,

    /// WSL distribution to use (when `runner_mode` is wsl)
    #[arg(long, global = true)]
    pub runner_distro: Option<String>,

    /// Path to Claude CLI binary in WSL
    #[arg(long, global = true)]
    pub claude_path: Option<String>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Tool patterns to allow (passed to Claude as allowedTools)
    #[arg(long, global = true)]
    pub allow: Vec<String>,

    /// Tool patterns to deny (passed to Claude as disallowedTools)
    #[arg(long, global = true)]
    pub deny: Vec<String>,

    /// Skip permission checks (passed to Claude)
    #[arg(long, global = true)]
    pub dangerously_skip_permissions: bool,

    /// Ignore specific secret patterns by pattern ID (e.g. github_pat, aws_access_key)
    #[arg(long, global = true)]
    pub ignore_secret_pattern: Vec<String>,

    /// Add extra secret patterns to detect (regex)
    #[arg(long, global = true)]
    pub extra_secret_pattern: Vec<String>,

    /// Phase timeout in seconds (default: 600, min: 5)
    #[arg(long, global = true)]
    pub phase_timeout: Option<u64>,

    /// Maximum bytes for stdout ring buffer (default: 2097152 = 2 MiB)
    #[arg(long, global = true)]
    pub stdout_cap_bytes: Option<usize>,

    /// Maximum bytes for stderr ring buffer (default: 262144 = 256 KiB)
    #[arg(long, global = true)]
    pub stderr_cap_bytes: Option<usize>,

    /// Lock TTL in seconds (default: 900 = 15 minutes)
    #[arg(long, global = true)]
    pub lock_ttl_seconds: Option<u64>,

    /// Write full packet to `context/<phase>-packet.txt` after secret scan passes
    #[arg(long, global = true)]
    pub debug_packet: bool,

    /// Allow symlinks and hardlinks in fixup targets
    #[arg(long, global = true)]
    pub allow_links: bool,

    /// Enable strict validation (validation failures fail phases)
    /// Use --no-strict-validation to disable strict mode
    #[arg(long, global = true, overrides_with = "no_strict_validation")]
    pub strict_validation: bool,

    /// Disable strict validation (validation failures log warnings only)
    /// Use --strict-validation to enable strict mode
    #[arg(long, global = true, overrides_with = "strict_validation")]
    pub no_strict_validation: bool,

    /// LLM provider to use (claude-cli)
    #[arg(long, global = true)]
    pub llm_provider: Option<String>,

    /// Fallback LLM provider to use if primary provider fails to initialize
    #[arg(long, global = true)]
    pub llm_fallback_provider: Option<String>,

    /// Prompt template to use for LLM interactions
    #[arg(long, global = true)]
    pub prompt_template: Option<String>,

    /// Path to Claude CLI binary (for claude-cli provider)
    #[arg(long, global = true)]
    pub llm_claude_binary: Option<String>,

    /// Path to Gemini CLI binary (for gemini-cli provider)
    #[arg(long, global = true)]
    pub llm_gemini_binary: Option<String>,

    /// Default Gemini model to use when no per-phase override is set
    #[arg(long, global = true)]
    pub llm_gemini_default_model: Option<String>,

    /// Execution strategy: controlled (default, LLMs cannot write directly)
    #[arg(long, global = true)]
    pub execution_strategy: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Generate a complete spec flow (Requirements → Design → Tasks)
    ///
    /// This command executes the full spec generation workflow, transforming
    /// a rough idea into detailed requirements, design, and implementation tasks.
    ///
    /// EXAMPLES:
    ///   echo "Build a calculator app" | xchecker spec calc-app
    ///   xchecker spec issue-42 --source gh --gh myorg/myrepo
    ///   xchecker spec new-feature --source fs --repo ./project --dry-run
    Spec {
        /// Unique identifier for the spec
        id: String,

        /// Source type: 'gh' (GitHub issue), 'fs' (filesystem), 'stdin' (standard input)
        #[arg(long, default_value = "stdin", value_parser = ["gh", "fs", "stdin"])]
        source: String,

        /// GitHub repository in format 'owner/repo' (required when --source gh)
        #[arg(long, help = "GitHub repository (e.g., 'myorg/myrepo')")]
        gh: Option<String>,

        /// Local repository path (required when --source fs)
        #[arg(long, help = "Path to local repository directory")]
        repo: Option<String>,

        /// Run in dry-run mode (show what would be executed without making LLM calls)
        #[arg(long)]
        dry_run: bool,

        /// Force override of stale locks
        #[arg(long)]
        force: bool,

        /// Apply fixups to files (default is preview mode)
        #[arg(long)]
        apply_fixups: bool,

        /// Hard fail on lockfile drift (exit with error if model/CLI version differs)
        #[arg(long)]
        strict_lock: bool,

        /// Output spec information as JSON (for Claude Code integration)
        #[arg(long)]
        json: bool,
    },

    /// Show status of a spec
    ///
    /// Displays the current state of a spec including completed phases,
    /// artifacts with BLAKE3 hashes, receipts, and configuration.
    ///
    /// EXAMPLES:
    ///   xchecker status my-spec
    ///   xchecker status my-spec --json
    Status {
        /// Spec ID to check status for
        id: String,

        /// Output status as JSON
        #[arg(long)]
        json: bool,
    },

    /// Resume execution from a specific phase
    ///
    /// Continues spec generation from a specific phase, useful for recovery
    /// from failures or re-running phases with different parameters.
    ///
    /// EXAMPLES:
    ///   xchecker resume my-spec --phase design
    ///   xchecker resume my-spec --phase requirements --dry-run
    ///   xchecker resume my-spec --phase design --json
    Resume {
        /// Spec ID to resume
        id: String,

        /// Phase to resume from: requirements, design, tasks, review, fixup, final
        #[arg(long, value_parser = ["requirements", "design", "tasks", "review", "fixup", "final"])]
        phase: String,

        /// Run in dry-run mode (show what would be executed without making LLM calls)
        #[arg(long)]
        dry_run: bool,

        /// Force override of stale locks
        #[arg(long)]
        force: bool,

        /// Apply fixups to files (default is preview mode)
        #[arg(long)]
        apply_fixups: bool,

        /// Hard fail on lockfile drift (exit with error if model/CLI version differs)
        #[arg(long)]
        strict_lock: bool,

        /// Output resume information as JSON (for Claude Code integration)
        #[arg(long)]
        json: bool,
    },

    /// Clean up spec artifacts and receipts
    ///
    /// Removes all artifacts, receipts, and context files for a spec.
    /// Use --hard to skip confirmation prompts.
    ///
    /// EXAMPLES:
    ///   xchecker clean my-spec
    ///   xchecker clean my-spec --hard --force
    Clean {
        /// Spec ID to clean
        id: String,

        /// Remove artifacts without confirmation
        #[arg(long)]
        hard: bool,

        /// Force removal even if lock is present
        #[arg(long)]
        force: bool,
    },

    /// Run performance benchmarks (NFR1 validation)
    ///
    /// Validates performance targets: empty run ≤ 5s, packetization ≤ 200ms for 100 files.
    /// Useful for regression testing and performance validation.
    ///
    /// EXAMPLES:
    ///   xchecker benchmark
    ///   xchecker benchmark --file-count 50 --iterations 10 --verbose
    ///   xchecker benchmark --json
    ///   xchecker benchmark --max-empty-run-secs 3.0 --max-packetization-ms 150.0
    Benchmark {
        /// Number of files to create for packetization benchmark
        #[arg(long, default_value = "100")]
        file_count: usize,

        /// Size of each test file in bytes
        #[arg(long, default_value = "1024")]
        file_size: usize,

        /// Number of benchmark iterations
        #[arg(long, default_value = "5")]
        iterations: usize,

        /// Output benchmark results as JSON
        #[arg(long)]
        json: bool,

        /// Maximum allowed empty run time in seconds (default: 5.0)
        #[arg(long)]
        max_empty_run_secs: Option<f64>,

        /// Maximum allowed packetization time in milliseconds per 100 files (default: 200.0)
        #[arg(long)]
        max_packetization_ms: Option<f64>,

        /// Maximum allowed RSS memory in MB (optional)
        #[arg(long)]
        max_rss_mb: Option<f64>,

        /// Maximum allowed commit memory in MB (Windows only, optional)
        #[arg(long)]
        max_commit_mb: Option<f64>,
    },

    /// Run integration smoke tests to validate all components
    ///
    /// Validates that all systems are properly integrated and working.
    /// Useful for development and CI/CD validation.
    ///
    /// EXAMPLES:
    ///   xchecker test
    ///   xchecker test --components --smoke --verbose
    Test {
        /// Run component validation tests
        #[arg(long)]
        components: bool,

        /// Run smoke tests
        #[arg(long)]
        smoke: bool,
    },

    /// Run environment health checks
    ///
    /// Validates that Claude CLI is installed, runner configuration is correct,
    /// write permissions are available, and configuration is valid.
    ///
    /// EXAMPLES:
    ///   xchecker doctor
    ///   xchecker doctor --json
    ///   xchecker doctor --strict-exit  # Treat warnings as failures
    Doctor {
        /// Output doctor results as JSON
        #[arg(long)]
        json: bool,

        /// Treat warnings as failures (exit non-zero on any warn or fail)
        #[arg(long)]
        strict_exit: bool,
    },

    /// Initialize a new spec with optional lockfile creation
    ///
    /// Creates the spec directory structure and optionally pins the model
    /// and Claude CLI version for reproducibility tracking.
    ///
    /// EXAMPLES:
    ///   xchecker init my-spec
    ///   xchecker init my-spec --create-lock
    ///   xchecker init my-spec --create-lock --model haiku
    Init {
        /// Spec ID to initialize
        id: String,

        /// Create a lockfile to pin model and CLI version
        #[arg(long)]
        create_lock: bool,
    },

    /// Manage workspace and multi-spec projects
    ///
    /// Workspace commands allow managing multiple specs within a project.
    /// A workspace is defined by a `workspace.yaml` file that contains
    /// metadata about registered specs.
    ///
    /// EXAMPLES:
    ///   xchecker project init my-project
    ///   xchecker project add-spec feature-auth --tag backend
    ///   xchecker project list
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Gate merges on xchecker receipts/status
    ///
    /// Evaluates a spec against a configurable policy to determine if it
    /// meets requirements for CI/CD gates. Returns exit code 0 on policy
    /// success, 1 on policy violation.
    ///
    /// EXAMPLES:
    ///   xchecker gate my-spec
    ///   xchecker gate my-spec --min-phase design
    ///   xchecker gate my-spec --fail-on-pending-fixups
    ///   xchecker gate my-spec --max-phase-age 7d
    ///   xchecker gate my-spec --policy .xchecker/policy.toml
    ///   xchecker gate my-spec --json
    ///
    /// Per FR-GATE (Requirements 4.5.1, 4.5.2, 4.5.3, 4.5.4)
    Gate {
        /// Spec ID to evaluate
        id: String,

        /// Policy file path (TOML)
        /// Defaults to .xchecker/policy.toml in the repo or ~/.config/xchecker/policy.toml
        #[arg(long)]
        policy: Option<PathBuf>,

        /// Minimum phase that must be completed (default: tasks)
        /// Valid phases: requirements, design, tasks, review, fixup, final
        #[arg(long)]
        min_phase: Option<String>,

        /// Fail if any pending fixups exist
        #[arg(long)]
        fail_on_pending_fixups: bool,

        /// Maximum age of the latest successful phase run (e.g., "7d", "24h", "30m")
        /// Failed receipts do not count towards age (prevents flapping phases from appearing fresh)
        #[arg(long)]
        max_phase_age: Option<String>,

        /// Output gate results as JSON (gate-json.v1 schema)
        #[arg(long)]
        json: bool,
    },

    /// Manage spec templates
    ///
    /// Templates provide predefined configurations and problem statements
    /// for common use cases, allowing you to bootstrap specs quickly.
    ///
    /// EXAMPLES:
    ///   xchecker template list
    ///   xchecker template init fullstack-nextjs my-app
    ///   xchecker template init rust-microservice my-service
    ///
    /// Per FR-TEMPLATES (Requirements 4.7.1, 4.7.2, 4.7.3)
    #[command(subcommand)]
    Template(TemplateCommands),
}

/// Project/workspace management subcommands
#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Initialize a new workspace in the current directory
    ///
    /// Creates a `workspace.yaml` file that marks this directory as a project root.
    /// The workspace can then be used to manage multiple specs.
    ///
    /// EXAMPLES:
    ///   xchecker project init my-project
    Init {
        /// Name for the workspace
        name: String,
    },

    /// Add a spec to the workspace
    ///
    /// Registers a spec in the workspace registry with optional tags.
    ///
    /// EXAMPLES:
    ///   xchecker project add-spec feature-auth
    ///   xchecker project add-spec feature-auth --tag backend --tag security
    AddSpec {
        /// Spec ID to add
        spec_id: String,

        /// Tags for categorization (can be specified multiple times)
        #[arg(long, short)]
        tag: Vec<String>,

        /// Force override if spec already exists
        #[arg(long)]
        force: bool,
    },

    /// List all specs in the workspace
    ///
    /// Shows all registered specs with their tags and status.
    ///
    /// EXAMPLES:
    ///   xchecker project list
    List {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },

    /// Show aggregated status for all specs in the workspace
    ///
    /// Displays status summary for all registered specs including phase summaries,
    /// counts of failed/pending/stale specs, and overall workspace health.
    ///
    /// EXAMPLES:
    ///   xchecker project status
    ///   xchecker project status --json
    Status {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,

        /// Output status as JSON (workspace-status-json.v1 schema)
        #[arg(long)]
        json: bool,
    },

    /// Show history timeline for a spec
    ///
    /// Displays timeline of phase progression, timestamps, and selected metrics
    /// including LLM token usage and fixup counts.
    ///
    /// EXAMPLES:
    ///   xchecker project history feature-auth
    ///   xchecker project history feature-auth --json
    ///
    /// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
    History {
        /// Spec ID to show history for
        spec_id: String,

        /// Output history as JSON (workspace-history-json.v1 schema)
        #[arg(long)]
        json: bool,
    },

    /// Launch interactive terminal UI for workspace overview
    ///
    /// Displays an interactive TUI showing specs list with tags and status,
    /// latest receipt summary per selected spec, pending fixups, error counts,
    /// and stale specs. The TUI is read-only (no destructive operations).
    ///
    /// Navigation:
    ///   - Arrow keys or j/k: Move selection up/down
    ///   - Enter: View details for selected spec
    ///   - Esc: Go back / close details
    ///   - q: Quit
    ///
    /// EXAMPLES:
    ///   xchecker project tui
    ///
    /// Per FR-WORKSPACE-TUI (Requirements 4.4.1, 4.4.2, 4.4.3)
    Tui {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

/// Template management subcommands
#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available built-in templates
    ///
    /// Shows all available templates with their descriptions and use cases.
    ///
    /// EXAMPLES:
    ///   xchecker template list
    ///
    /// Per FR-TEMPLATES (Requirements 4.7.1)
    List,

    /// Initialize a spec from a template
    ///
    /// Creates a new spec with predefined problem statement, configuration,
    /// and example partial spec flow based on the selected template.
    ///
    /// Available templates:
    ///   - fullstack-nextjs: Full-stack web applications with Next.js
    ///   - rust-microservice: Rust microservices and CLI tools
    ///   - python-fastapi: Python REST APIs with FastAPI
    ///   - docs-refactor: Documentation improvements and refactoring
    ///
    /// EXAMPLES:
    ///   xchecker template init fullstack-nextjs my-app
    ///   xchecker template init rust-microservice my-service
    ///   xchecker template init python-fastapi my-api
    ///   xchecker template init docs-refactor docs-v2
    ///
    /// Per FR-TEMPLATES (Requirements 4.7.2, 4.7.3)
    Init {
        /// Template to use (fullstack-nextjs, rust-microservice, python-fastapi, docs-refactor)
        template: String,

        /// Spec ID to create
        spec_id: String,
    },
}

/// Build the CLI command structure without parsing arguments
/// This is used for introspection in tests and documentation validation
#[must_use]
pub fn build_cli() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}

/// Main CLI execution function.
///
/// This function handles ALL output including errors. It returns `Result<(), ExitCode>`:
/// - On success: returns `Ok(())` after printing any output
/// - On error: prints error message via contextual reporting, returns `Err(ExitCode)`
///
/// main.rs only calls `std::process::exit(code.as_i32())` on error - it does NOT print.
///
/// _Requirements: FR-CLI-3, FR-CLI-4_
pub fn run() -> Result<(), ExitCode> {
    let cli = Cli::parse();

    // Build CLI args for configuration system (wired through build_cli)
    let cli_args = CliArgs {
        config_path: cli.config.clone(),
        model: cli.model.clone(),
        max_turns: cli.max_turns,
        packet_max_bytes: cli.packet_max_bytes,
        packet_max_lines: cli.packet_max_lines,
        output_format: cli.output_format.clone(),
        verbose: Some(cli.verbose),
        runner_mode: cli.runner_mode.clone(),
        runner_distro: cli.runner_distro.clone(),
        claude_path: cli.claude_path.clone(),
        allow: cli.allow.clone(),
        deny: cli.deny.clone(),
        dangerously_skip_permissions: cli.dangerously_skip_permissions,
        ignore_secret_pattern: cli.ignore_secret_pattern.clone(),
        extra_secret_pattern: cli.extra_secret_pattern.clone(),
        phase_timeout: cli.phase_timeout,
        stdout_cap_bytes: cli.stdout_cap_bytes,
        stderr_cap_bytes: cli.stderr_cap_bytes,
        lock_ttl_seconds: cli.lock_ttl_seconds,
        debug_packet: cli.debug_packet,
        allow_links: cli.allow_links,
        strict_validation: if cli.strict_validation {
            Some(true)
        } else if cli.no_strict_validation {
            Some(false)
        } else {
            None
        },
        llm_provider: cli.llm_provider.clone(),
        llm_fallback_provider: cli.llm_fallback_provider.clone(),
        prompt_template: cli.prompt_template.clone(),
        llm_claude_binary: cli.llm_claude_binary.clone(),
        llm_gemini_binary: cli.llm_gemini_binary.clone(),
        llm_gemini_default_model: cli.llm_gemini_default_model.clone(),
        execution_strategy: cli.execution_strategy.clone(),
    };

    // Discover and load configuration
    let config = match Config::discover(&cli_args) {
        Ok(config) => config,
        Err(err) => {
            let contextual_report = error_utils::create_contextual_report(&err, "config");
            eprintln!("{contextual_report}");
            return Err(err.to_exit_code());
        }
    };

    // Build a configured redactor once from the effective config so all output surfaces
    // respect extra/ignore patterns (FR-SEC-19).
    let redactor = match SecretRedactor::from_config(&config) {
        Ok(redactor) => Arc::new(redactor),
        Err(e) => {
            let err = XCheckerError::Config(ConfigError::InvalidValue {
                key: "security".to_string(),
                value: e.to_string(),
            });
            let contextual_report = error_utils::create_contextual_report(&err, "config");
            eprintln!("{contextual_report}");
            return Err(err.to_exit_code());
        }
    };

    // Create tokio runtime for async operations
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("✗ Failed to create async runtime: {e}");
            return Err(ExitCode::INTERNAL);
        }
    };

    // Determine operation context for better error reporting before moving cli.command
    let operation = match &cli.command {
        Commands::Spec { .. } => "spec",
        Commands::Status { .. } => "status",
        Commands::Resume { .. } => "resume",
        Commands::Clean { .. } => "clean",
        Commands::Benchmark { .. } => "benchmark",
        Commands::Test { .. } => "test",
        Commands::Doctor { .. } => "doctor",
        Commands::Init { .. } => "init",
        Commands::Project(_) => "project",
        Commands::Gate { .. } => "gate",
        Commands::Template(_) => "template",
    };

    let result = rt.block_on(async {
        match cli.command {
            Commands::Spec {
                id,
                source,
                gh,
                repo,
                dry_run,
                force,
                apply_fixups,
                strict_lock,
                json,
            } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;

                // If --json flag is set, output spec info as JSON and return
                if json {
                    return execute_spec_json_command(&sanitized_id, &config);
                }

                execute_spec_command(
                    &sanitized_id,
                    &source,
                    gh.as_deref(),
                    repo.as_deref(),
                    dry_run,
                    cli.verbose,
                    force,
                    apply_fixups,
                    strict_lock,
                    &config,
                    &cli_args,
                    &redactor,
                )
                .await
            }
            Commands::Status { id, json } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;
                execute_status_command(&sanitized_id, json, &config)
            }
            Commands::Resume {
                id,
                phase,
                dry_run,
                force,
                apply_fixups,
                strict_lock,
                json,
            } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;

                // If --json flag is set, output resume info as JSON and return
                if json {
                    return execute_resume_json_command(&sanitized_id, &phase, &config);
                }

                execute_resume_command(
                    &sanitized_id,
                    &phase,
                    dry_run,
                    cli.verbose,
                    force,
                    apply_fixups,
                    strict_lock,
                    &config,
                    &cli_args,
                    &redactor,
                )
                .await
            }
            Commands::Clean { id, hard, force } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;
                execute_clean_command(&sanitized_id, hard, force, &config)
            }
            Commands::Benchmark {
                file_count,
                file_size,
                iterations,
                json,
                max_empty_run_secs,
                max_packetization_ms,
                max_rss_mb,
                max_commit_mb,
            } => execute_benchmark_command(
                file_count,
                file_size,
                iterations,
                json,
                max_empty_run_secs,
                max_packetization_ms,
                max_rss_mb,
                max_commit_mb,
                cli.verbose,
            ),
            Commands::Test { components, smoke } => {
                execute_test_command(components, smoke, cli.verbose)
            }
            Commands::Doctor { json, strict_exit } => {
                execute_doctor_command(json, strict_exit, &config)
            }
            Commands::Init { id, create_lock } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;
                execute_init_command(&sanitized_id, create_lock, &config)
            }
            Commands::Project(project_cmd) => execute_project_command(project_cmd),
            Commands::Gate {
                id,
                policy,
                min_phase,
                fail_on_pending_fixups,
                max_phase_age,
                json,
            } => {
                // Sanitize spec ID (R5.7)
                let sanitized_id = sanitize_spec_id(&id).map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "spec_id".to_string(),
                        value: format!("{e}"),
                    })
                })?;
                execute_gate_command(
                    &sanitized_id,
                    policy.as_deref(),
                    min_phase.as_deref(),
                    fail_on_pending_fixups,
                    max_phase_age.as_deref(),
                    json,
                )
            }
            Commands::Template(template_cmd) => execute_template_command(template_cmd),
        }
    });

    // Handle errors with structured reporting (R1.3, R6.4, R6.8, R6.9)
    // cli::run() handles ALL output including errors (FR-CLI-3, FR-CLI-6)
    // Error messages are displayed via contextual reporting which extends display_for_user()
    // with operation-specific context for better user experience
    if let Err(error) = result {
        // Try to downcast to XCheckerError for better reporting
        if let Some(xchecker_error) = error.downcast_ref::<XCheckerError>() {
            // Use contextual error reporting which builds on display_for_user() (FR-CLI-3)
            // This provides user_message(), context(), suggestions() plus operation-specific help
            let contextual_report = error_utils::create_contextual_report_with_redactor(
                xchecker_error,
                operation,
                redactor.as_ref(),
            );
            eprintln!("{contextual_report}");

            // Return the appropriate exit code - main.rs will call std::process::exit()
            return Err(xchecker_error.to_exit_code());
        } else {
            // Fallback for other error types with enhanced context
            let redacted_error = redactor.redact_string(&error.to_string());
            eprintln!("✗ Unexpected error: {redacted_error}");

            // Provide enhanced context and suggestions for common anyhow errors
            if let Some(suggestions) = enhance_error_context(&error) {
                eprintln!("\n  Suggestions:");
                for (i, suggestion) in suggestions.iter().enumerate() {
                    eprintln!("    {}. {}", i + 1, suggestion);
                }
            }

            // Provide general troubleshooting steps
            eprintln!("\n  General troubleshooting:");
            eprintln!("    - Run with --verbose for more detailed output");
            eprintln!("    - Check the xchecker documentation for common issues");
            eprintln!("    - Ensure all dependencies are properly installed");

            return Err(ExitCode::INTERNAL);
        }
    }

    Ok(())
}

/// Execute the spec generation command
#[allow(clippy::too_many_arguments)]
async fn execute_spec_command(
    spec_id: &str,
    source_type: &str,
    gh_repo: Option<&str>,
    fs_repo: Option<&str>,
    dry_run: bool,
    verbose: bool,
    force: bool,
    apply_fixups: bool,
    strict_lock: bool,
    config: &Config,
    cli_args: &CliArgs,
    redactor: &Arc<SecretRedactor>,
) -> Result<()> {
    // Create logger for verbose output and timing (R7.5, NFR5)
    let mut logger = Logger::new(verbose);
    logger.start_timing("total_execution");

    logger.verbose(&format!("Starting spec generation for ID: {spec_id}"));
    if dry_run {
        logger.verbose("Running in dry-run mode (no Claude calls will be made)");
    }

    // Resolve source input (R6.4)
    logger.start_timing("source_resolution");
    let source_content = match source_type {
        "gh" => {
            let gh_repo = gh_repo.ok_or_else(|| {
                XCheckerError::Config(ConfigError::MissingRequired("--gh owner/repo".to_string()))
            })?;

            let parts: Vec<&str> = gh_repo.split('/').collect();
            if parts.len() != 2 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "gh".to_string(),
                    value: gh_repo.to_string(),
                })
                .into());
            }

            SourceResolver::resolve_github(parts[0], parts[1], spec_id).map_err(|_e| {
                // Convert SourceError to XCheckerError for consistent reporting (R6.4)
                XCheckerError::Source(crate::error::SourceError::GitHubRepoNotFound {
                    owner: parts[0].to_string(),
                    repo: parts[1].to_string(),
                })
            })?
        }
        "fs" => {
            let fs_repo = fs_repo.ok_or_else(|| {
                XCheckerError::Config(ConfigError::MissingRequired("--repo <path>".to_string()))
            })?;

            let path = PathBuf::from(fs_repo);
            SourceResolver::resolve_filesystem(&path).map_err(|e| {
                // Enhanced error reporting for filesystem source resolution (R6.4)
                if path.exists() {
                    if path.is_dir() {
                        // Check if it's a permission issue or other access problem
                        match std::fs::read_dir(&path) {
                            Err(io_err)
                                if io_err.kind() == std::io::ErrorKind::PermissionDenied =>
                            {
                                XCheckerError::Source(
                                    crate::error::SourceError::FileSystemAccessDenied {
                                        path: fs_repo.to_string(),
                                    },
                                )
                            }
                            Err(_) => {
                                XCheckerError::Source(crate::error::SourceError::InvalidFormat {
                                    reason: format!("Directory exists but cannot be read: {e}"),
                                })
                            }
                            Ok(_) => {
                                // Directory is readable, so it's some other issue
                                XCheckerError::Source(crate::error::SourceError::InvalidFormat {
                                    reason: format!("Failed to resolve filesystem source: {e}"),
                                })
                            }
                        }
                    } else {
                        XCheckerError::Source(crate::error::SourceError::FileSystemNotDirectory {
                            path: fs_repo.to_string(),
                        })
                    }
                } else {
                    XCheckerError::Source(crate::error::SourceError::FileSystemNotFound {
                        path: fs_repo.to_string(),
                    })
                }
            })?
        }
        "stdin" => {
            SourceResolver::resolve_stdin().map_err(|e| {
                // Enhanced error reporting for stdin source resolution (R6.4)
                let error_msg = e.to_string();
                if error_msg.contains("empty") || error_msg.contains("EOF") {
                    XCheckerError::Source(crate::error::SourceError::EmptyInput)
                } else if error_msg.contains("permission") || error_msg.contains("access") {
                    XCheckerError::Source(crate::error::SourceError::StdinReadFailed {
                        reason: "Permission denied or stdin not accessible".to_string(),
                    })
                } else {
                    XCheckerError::Source(crate::error::SourceError::StdinReadFailed {
                        reason: error_msg,
                    })
                }
            })?
        }
        _ => {
            return Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "source".to_string(),
                value: format!("Unknown source type '{source_type}'. Valid options: 'gh' (GitHub), 'fs' (filesystem), 'stdin' (standard input)"),
            }).into());
        }
    };
    logger.end_timing("source_resolution");

    // Extract problem statement from resolved source
    let problem_statement = source_content.content.clone();
    logger.verbose(&format!("Source resolved successfully from: {source_type}"));

    // Persist problem statement to spec directory (FR-PKT: problem statement in packet)
    // This ensures the problem statement is available for packet building
    let spec_root = crate::paths::spec_root(spec_id);
    let source_dir = spec_root.join("source");
    crate::paths::ensure_dir_all(&source_dir)
        .with_context(|| format!("Failed to create source directory: {}", source_dir))?;

    let problem_path = source_dir.join("00-problem-statement.md");
    write_file_atomic(
        &problem_path,
        &format!("# Problem Statement\n\n{}\n", problem_statement.trim()),
    )
    .with_context(|| format!("Failed to write problem statement: {}", problem_path))?;

    logger.verbose(&format!("Problem statement written to: {}", problem_path));

    // Check for lockfile drift (R10.2, R10.4)
    let model_full_name = config.defaults.model.as_deref().unwrap_or("haiku");
    let claude_cli_version = detect_claude_cli_version().unwrap_or_else(|_| "unknown".to_string());
    let _lock_drift =
        check_lockfile_drift(spec_id, strict_lock, model_full_name, &claude_cli_version)?;

    // Configure execution using shared helper, passing problem statement for prompt construction
    let orchestrator_config = build_orchestrator_config(
        dry_run,
        verbose,
        apply_fixups,
        config,
        cli_args,
        Some(&problem_statement),
        redactor.clone(),
    );

    // Create orchestrator handle (this will acquire the file lock)
    logger.start_timing("orchestrator_setup");
    let mut handle = OrchestratorHandle::with_config_and_force(spec_id, orchestrator_config, force)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;
    logger.end_timing("orchestrator_setup");

    logger.verbose("Executing Requirements phase...");

    // Execute Requirements phase
    logger.start_timing("requirements_phase");
    let result = handle
        .run_phase(PhaseId::Requirements)
        .await
        .with_context(|| "Failed to execute Requirements phase")?;
    logger.end_timing("requirements_phase");

    // Report results
    logger.end_timing("total_execution");

    if result.success {
        println!("✓ Requirements phase completed successfully");

        logger.verbose(&format!("Phase: {}", result.phase.as_str()));
        logger.verbose(&format!("Exit code: {}", result.exit_code));
        logger.verbose(&format!(
            "Artifacts created: {}",
            result.artifact_paths.len()
        ));

        for (i, path) in result.artifact_paths.iter().enumerate() {
            logger.verbose(&format!("  {}: {}", i + 1, path.display()));
        }

        if let Some(receipt_path) = &result.receipt_path {
            logger.verbose(&format!("Receipt: {}", receipt_path.display()));
        }

        // Print performance summary if verbose (R7.5, NFR5)
        logger.print_performance_summary();

        // Show next steps
        println!("\nNext steps:");
        println!("  - Review the generated requirements in .xchecker/specs/{spec_id}/artifacts/");
        println!("  - Check status with: xchecker status {spec_id}");
        println!("  - Continue to Design phase: xchecker resume {spec_id} --phase design");
    } else {
        // Create structured error for phase failure (R1.3, R4.3)
        let phase_error = PhaseError::ExecutionFailed {
            phase: result.phase.as_str().to_string(),
            code: result.exit_code,
        };
        let xchecker_error = XCheckerError::Phase(phase_error);

        // Report with full context and suggestions
        let report = ErrorReport::new(&xchecker_error);
        eprintln!("{}", report.format_with_redactor(redactor.as_ref()));

        // Enhanced error reporting for phase failures (R1.3, R4.3)
        if let Some(error_msg) = &result.error {
            let redacted_error_msg = redactor.redact_string(error_msg);
            eprintln!("\n  Phase failure details: {redacted_error_msg}");

            // Try to parse the error for enhanced reporting
            if error_msg.contains("ExecutionFailedWithStderr") {
                eprintln!("  ↳ Claude CLI produced error output (see receipt for full stderr)");
            } else if error_msg.contains("PartialOutputSaved") {
                eprintln!("  ↳ Partial output was saved for debugging");
            }
        }

        // Show partial artifacts location (R4.3)
        eprintln!("\n  Debugging information:");
        if !result.artifact_paths.is_empty() {
            eprintln!("    Partial artifacts:");
            for path in &result.artifact_paths {
                eprintln!("      - {}", path.display());
            }
        }
        eprintln!("    Spec directory: .xchecker/specs/{spec_id}/");

        if let Some(receipt_path) = &result.receipt_path {
            eprintln!("    Execution receipt: {}", receipt_path.display());
            eprintln!("      ↳ Contains stderr output, warnings, and execution metadata");
        }

        // Provide recovery suggestions
        eprintln!("\n  Recovery options:");
        eprintln!("    - Review partial outputs and receipt for error details");
        eprintln!("    - Fix any configuration or connectivity issues");
        eprintln!("    - Retry with: xchecker spec {spec_id}");
        eprintln!("    - Test configuration with: xchecker spec {spec_id} --dry-run");

        std::process::exit(result.exit_code);
    }

    Ok(())
}

/// Execute the spec --json command (FR-Claude Code-CLI: Claude Code CLI Surfaces)
/// Returns JSON with schema_version, spec_id, phases, config_summary
/// Excludes full artifacts and packet contents per Requirements 4.1.1, 4.1.4
fn execute_spec_json_command(spec_id: &str, config: &Config) -> Result<()> {
    use crate::types::{PhaseId, PhaseInfo, SpecConfigSummary, SpecOutput};

    // Create read-only handle to access managers (no lock needed for JSON output)
    let handle = OrchestratorHandle::readonly(spec_id)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;

    // Check if spec directory exists
    let base_path = handle.artifact_manager().base_path();
    if !base_path.exists() {
        // Return minimal JSON for non-existent spec
        let output = SpecOutput {
            schema_version: "spec-json.v1".to_string(),
            spec_id: spec_id.to_string(),
            phases: vec![],
            config_summary: SpecConfigSummary {
                execution_strategy: config
                    .llm
                    .execution_strategy
                    .clone()
                    .unwrap_or_else(|| "controlled".to_string()),
                provider: config.llm.provider.clone(),
                spec_path: base_path.to_string(),
            },
        };
        let json_output = emit_spec_json(&output)?;
        println!("{json_output}");
        return Ok(());
    }

    // Get phase information
    let all_phases = [
        PhaseId::Requirements,
        PhaseId::Design,
        PhaseId::Tasks,
        PhaseId::Review,
        PhaseId::Fixup,
        PhaseId::Final,
    ];

    // Get receipts to determine phase status and last run times
    // Handle case where receipts directory doesn't exist yet
    let receipts = handle.receipt_manager().list_receipts().unwrap_or_default();

    // Build phase info list
    let mut phases = Vec::new();
    for phase_id in &all_phases {
        let phase_completed = handle.artifact_manager().phase_completed(*phase_id);

        // Find the latest receipt for this phase
        let latest_receipt = receipts
            .iter()
            .filter(|r| r.phase == phase_id.as_str())
            .max_by_key(|r| r.emitted_at);

        let status = if phase_completed {
            "completed".to_string()
        } else if latest_receipt.is_some() {
            "pending".to_string()
        } else {
            "not_started".to_string()
        };

        let last_run = latest_receipt.map(|r| r.emitted_at);

        phases.push(PhaseInfo {
            phase_id: phase_id.as_str().to_string(),
            status,
            last_run,
        });
    }

    // Build config summary (excludes full artifacts and packet contents)
    let config_summary = SpecConfigSummary {
        execution_strategy: config
            .llm
            .execution_strategy
            .clone()
            .unwrap_or_else(|| "controlled".to_string()),
        provider: config.llm.provider.clone(),
        spec_path: base_path.to_string(),
    };

    let output = SpecOutput {
        schema_version: "spec-json.v1".to_string(),
        spec_id: spec_id.to_string(),
        phases,
        config_summary,
    };

    let json_output = emit_spec_json(&output)?;
    println!("{json_output}");

    Ok(())
}

/// Emit spec output as canonical JSON using JCS (RFC 8785)
fn emit_spec_json(output: &crate::types::SpecOutput) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit spec JSON")
}

/// Emit status output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.2): Returns compact status summary
fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit status JSON")
}

/// Execute the resume --json command (FR-Claude Code-CLI: Claude Code CLI Surfaces)
/// Returns JSON with schema_version, spec_id, phase, current_inputs, next_steps
/// Excludes full packet and raw artifacts per Requirements 4.1.3, 4.1.4
fn execute_resume_json_command(spec_id: &str, phase_name: &str, config: &Config) -> Result<()> {
    use crate::types::{CurrentInputs, PhaseId, ResumeJsonOutput};

    // Parse phase name
    let phase_id = match phase_name.to_lowercase().as_str() {
        "requirements" => PhaseId::Requirements,
        "design" => PhaseId::Design,
        "tasks" => PhaseId::Tasks,
        "review" => PhaseId::Review,
        "fixup" => PhaseId::Fixup,
        "final" => PhaseId::Final,
        _ => {
            return Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "phase".to_string(),
                value: format!("Unknown phase '{phase_name}'. Valid phases: requirements, design, tasks, review, fixup, final"),
            }).into());
        }
    };

    // Create read-only handle to access managers (no lock needed for JSON output)
    let handle = OrchestratorHandle::readonly(spec_id)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;

    // Check if spec directory exists
    let base_path = handle.artifact_manager().base_path();
    let spec_exists = base_path.exists();

    // Get available artifacts (names only, not contents)
    let available_artifacts = if spec_exists {
        handle
            .artifact_manager()
            .list_artifacts()
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Get latest completed phase
    let latest_completed_phase = if spec_exists {
        handle
            .artifact_manager()
            .get_latest_completed_phase()
            .map(|p| p.as_str().to_string())
    } else {
        None
    };

    // Build current inputs (high-level metadata only, no full contents)
    let current_inputs = CurrentInputs {
        available_artifacts,
        spec_exists,
        latest_completed_phase,
    };

    // Generate next steps hint based on phase and current state
    let next_steps = generate_next_steps_hint(spec_id, phase_id, &current_inputs, config);

    let output = ResumeJsonOutput {
        schema_version: "resume-json.v1".to_string(),
        spec_id: spec_id.to_string(),
        phase: phase_id.as_str().to_string(),
        current_inputs,
        next_steps,
    };

    let json_output = emit_resume_json(&output)?;
    println!("{json_output}");

    Ok(())
}

/// Generate next steps hint for resume JSON output
fn generate_next_steps_hint(
    spec_id: &str,
    phase_id: PhaseId,
    current_inputs: &crate::types::CurrentInputs,
    _config: &Config,
) -> String {
    if !current_inputs.spec_exists {
        return format!(
            "Spec '{}' does not exist. Run 'xchecker spec {}' to create it first.",
            spec_id, spec_id
        );
    }

    // Check if we have the required inputs for this phase
    let has_requirements = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("requirements"));
    let has_design = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("design"));
    let has_tasks = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("tasks"));
    let has_review = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("review"));

    match phase_id {
        PhaseId::Requirements => {
            "Run requirements phase to generate initial requirements from the problem statement."
                .to_string()
        }
        PhaseId::Design => {
            if has_requirements {
                "Run design phase to generate architecture and design from requirements."
                    .to_string()
            } else {
                format!(
                    "Requirements phase not completed. Run 'xchecker resume {} --phase requirements' first.",
                    spec_id
                )
            }
        }
        PhaseId::Tasks => {
            if has_design {
                "Run tasks phase to generate implementation tasks from design.".to_string()
            } else {
                format!(
                    "Design phase not completed. Run 'xchecker resume {} --phase design' first.",
                    spec_id
                )
            }
        }
        PhaseId::Review => {
            if has_tasks {
                "Run review phase to review and validate the generated spec.".to_string()
            } else {
                format!(
                    "Tasks phase not completed. Run 'xchecker resume {} --phase tasks' first.",
                    spec_id
                )
            }
        }
        PhaseId::Fixup => {
            if has_review {
                "Run fixup phase to apply any suggested changes from review.".to_string()
            } else {
                format!(
                    "Review phase not completed. Run 'xchecker resume {} --phase review' first.",
                    spec_id
                )
            }
        }
        PhaseId::Final => "Run final phase to complete the spec generation workflow.".to_string(),
    }
}

/// Emit resume output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.3): Returns resume context without full packet/artifacts
fn emit_resume_json(output: &crate::types::ResumeJsonOutput) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit resume JSON")
}

/// Count pending fixups for a spec
/// Returns the number of target files with pending fixups
fn count_pending_fixups(handle: &OrchestratorHandle) -> u32 {
    crate::fixup::pending_fixups_from_handle(handle).targets
}

/// Execute the status command
fn execute_status_command(spec_id: &str, json: bool, config: &Config) -> Result<()> {
    // Create read-only handle to access managers (no lock needed for status)
    let handle = OrchestratorHandle::readonly(spec_id)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;

    // Check if spec directory exists
    let base_path = handle.artifact_manager().base_path();
    if !base_path.exists() {
        if json {
            // Return empty JSON for non-existent spec
            println!("{{}}");
        } else {
            println!("Status for spec: {spec_id}");
            println!("  Status: No spec found");
            println!("  Directory: {base_path} (does not exist)");
        }
        return Ok(());
    }

    // If JSON output is requested, use status-json.v2 format with full details
    // Includes artifacts with blake3_first8, effective_config, and lock_drift
    if json {
        use crate::lock::{RunContext, XCheckerLock};
        use crate::types::{
            ArtifactInfo, ConfigSource, ConfigValue, PhaseStatusInfo, StatusJsonOutput,
        };
        use std::collections::BTreeMap;

        // Get all phases
        let all_phases = [
            PhaseId::Requirements,
            PhaseId::Design,
            PhaseId::Tasks,
            PhaseId::Review,
            PhaseId::Fixup,
            PhaseId::Final,
        ];

        // Get receipts to determine phase status and receipt IDs
        let receipts = handle.receipt_manager().list_receipts().unwrap_or_default();

        // Build phase status list
        let mut phase_statuses = Vec::new();
        let mut has_errors = false;

        for phase_id in &all_phases {
            // Find the latest receipt for this phase
            let latest_receipt = receipts
                .iter()
                .filter(|r| r.phase == phase_id.as_str())
                .max_by_key(|r| r.emitted_at);

            let (status, receipt_id) = if let Some(receipt) = latest_receipt {
                // Check if the phase succeeded or failed
                if receipt.exit_code == 0 {
                    (
                        "success".to_string(),
                        Some(format!(
                            "{}-{}",
                            receipt.phase,
                            receipt.emitted_at.format("%Y%m%d_%H%M%S")
                        )),
                    )
                } else {
                    has_errors = true;
                    (
                        "failed".to_string(),
                        Some(format!(
                            "{}-{}",
                            receipt.phase,
                            receipt.emitted_at.format("%Y%m%d_%H%M%S")
                        )),
                    )
                }
            } else {
                ("not_started".to_string(), None)
            };

            phase_statuses.push(PhaseStatusInfo {
                phase_id: phase_id.as_str().to_string(),
                status,
                receipt_id,
            });
        }

        // Count pending fixups
        let pending_fixups = count_pending_fixups(&handle);

        // Collect artifacts with blake3_first8 from receipts
        let mut artifact_hashes: BTreeMap<String, String> = BTreeMap::new();
        for receipt in &receipts {
            for output in &receipt.outputs {
                // Extract just the filename from the path for matching
                if let Some(filename) = output.path.split('/').next_back() {
                    let short_hash = if output.blake3_canonicalized.len() >= 8 {
                        &output.blake3_canonicalized[..8]
                    } else {
                        &output.blake3_canonicalized
                    };
                    artifact_hashes.insert(filename.to_string(), short_hash.to_string());
                }
            }
        }

        // Build artifact info list
        let artifact_files = handle
            .artifact_manager()
            .list_artifacts()
            .unwrap_or_default();

        let mut artifacts: Vec<ArtifactInfo> = artifact_files
            .iter()
            .filter_map(|filename| {
                artifact_hashes.get(filename).map(|hash| ArtifactInfo {
                    path: format!("artifacts/{filename}"),
                    blake3_first8: hash.clone(),
                })
            })
            .collect();
        artifacts.sort_by(|a, b| a.path.cmp(&b.path));

        // Build effective_config from config with source attribution
        let mut effective_config: BTreeMap<String, ConfigValue> = BTreeMap::new();

        // Add key configuration values with their sources
        // Provider
        if let Some(ref provider) = config.llm.provider {
            let source = config
                .source_attribution
                .get("provider")
                .cloned()
                .unwrap_or(ConfigSource::Config);
            effective_config.insert(
                "provider".to_string(),
                ConfigValue {
                    value: serde_json::Value::String(provider.clone()),
                    source,
                },
            );
        }

        // Model
        if let Some(ref model) = config.defaults.model {
            let source = config
                .source_attribution
                .get("model")
                .cloned()
                .unwrap_or(ConfigSource::Config);
            effective_config.insert(
                "model".to_string(),
                ConfigValue {
                    value: serde_json::Value::String(model.clone()),
                    source,
                },
            );
        }

        // Max turns
        if let Some(max_turns) = config.defaults.max_turns {
            let source = config
                .source_attribution
                .get("max_turns")
                .cloned()
                .unwrap_or(ConfigSource::Config);
            effective_config.insert(
                "max_turns".to_string(),
                ConfigValue {
                    value: serde_json::Value::Number(max_turns.into()),
                    source,
                },
            );
        }

        // Phase timeout
        if let Some(timeout) = config.defaults.phase_timeout {
            let source = config
                .source_attribution
                .get("phase_timeout")
                .cloned()
                .unwrap_or(ConfigSource::Config);
            effective_config.insert(
                "phase_timeout".to_string(),
                ConfigValue {
                    value: serde_json::Value::Number(timeout.into()),
                    source,
                },
            );
        }

        // Execution strategy
        if let Some(ref strategy) = config.llm.execution_strategy {
            let source = config
                .source_attribution
                .get("execution_strategy")
                .cloned()
                .unwrap_or(ConfigSource::Config);
            effective_config.insert(
                "execution_strategy".to_string(),
                ConfigValue {
                    value: serde_json::Value::String(strategy.clone()),
                    source,
                },
            );
        }

        // Load lockfile and detect drift
        let lock_drift = if let Ok(Some(lock)) = XCheckerLock::load(spec_id) {
            // Get current run context from latest receipt or config
            let model_full_name = receipts
                .last()
                .map(|r| r.model_full_name.clone())
                .unwrap_or_else(|| config.defaults.model.clone().unwrap_or_default());

            let claude_cli_version = receipts
                .last()
                .map(|r| r.claude_cli_version.clone())
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

            let context = RunContext {
                model_full_name,
                claude_cli_version,
                schema_version: "1".to_string(),
            };

            lock.detect_drift(&context)
        } else {
            None
        };

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: spec_id.to_string(),
            phase_statuses,
            pending_fixups,
            has_errors,
            strict_validation: config.strict_validation(),
            artifacts,
            effective_config,
            lock_drift,
        };

        // Emit as canonical JSON using JCS (RFC 8785)
        let json_output =
            emit_status_json(&output).with_context(|| "Failed to emit status JSON")?;

        println!("{json_output}");
        return Ok(());
    }

    // Human-readable output
    println!("Status for spec: {spec_id}");
    println!("  Directory: {base_path}");

    // Get latest completed phase and show phase progression (R2.6)
    let latest_completed = handle.artifact_manager().get_latest_completed_phase();
    match latest_completed {
        Some(phase) => {
            println!("  Latest completed phase: {}", phase.as_str());
        }
        None => {
            println!("  Latest completed phase: None");
        }
    }

    // List artifacts with first-8 BLAKE3 hashes (R2.6, R8.1)
    let artifacts = handle
        .artifact_manager()
        .list_artifacts()
        .with_context(|| "Failed to list artifacts")?;

    if artifacts.is_empty() {
        println!("  Artifacts: None");
    } else {
        println!("  Artifacts: {} found", artifacts.len());

        // Get receipts to extract hashes for artifacts
        let receipts = handle
            .receipt_manager()
            .list_receipts()
            .with_context(|| "Failed to list receipts")?;

        // Create a map of artifact paths to their hashes from receipts
        let mut artifact_hashes: HashMap<String, String> = HashMap::new();
        for receipt in &receipts {
            for output in &receipt.outputs {
                // Extract just the filename from the path for matching
                if let Some(filename) = output.path.split('/').next_back() {
                    let short_hash = if output.blake3_canonicalized.len() >= 8 {
                        &output.blake3_canonicalized[..8]
                    } else {
                        &output.blake3_canonicalized
                    };
                    artifact_hashes.insert(filename.to_string(), short_hash.to_string());
                }
            }
        }

        for artifact in &artifacts {
            if let Some(hash) = artifact_hashes.get(artifact) {
                println!("    - {artifact} -> {hash}");
            } else {
                println!("    - {artifact} -> <no hash>");
            }
        }
    }

    // Display last receipt path and key information (R2.6, R8.2)
    let receipts = handle
        .receipt_manager()
        .list_receipts()
        .with_context(|| "Failed to list receipts")?;

    if receipts.is_empty() {
        println!("  Last receipt: None");
    } else {
        let latest_receipt = receipts.last().unwrap();

        // Show receipt path
        let receipt_filename = format!(
            "{}-{}.json",
            latest_receipt.phase,
            latest_receipt.emitted_at.format("%Y%m%d_%H%M%S")
        );
        let receipt_path = base_path.join("receipts").join(receipt_filename);
        println!("  Last receipt: {receipt_path}");

        // Show key receipt information
        println!("    Phase: {}", latest_receipt.phase);
        println!(
            "    Emitted at: {}",
            latest_receipt.emitted_at.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("    Exit code: {}", latest_receipt.exit_code);
        println!("    Model: {}", latest_receipt.model_full_name);
        if let Some(alias) = &latest_receipt.model_alias {
            println!("    Model alias: {alias}");
        }
        println!("    Runner: {}", latest_receipt.runner);
        if let Some(distro) = &latest_receipt.runner_distro {
            println!("    Runner distro: {distro}");
        }
        println!(
            "    Canonicalization: {}",
            latest_receipt.canonicalization_version
        );

        if !latest_receipt.warnings.is_empty() {
            println!("    Warnings: {}", latest_receipt.warnings.len());
            for warning in &latest_receipt.warnings {
                println!("      - {warning}");
            }
        }

        if latest_receipt.fallback_used == Some(true) {
            println!("    Output format fallback: Used (stream-json → text)");
        }
    }

    // Show effective configuration with source attribution (R11.3)
    println!("\n  Effective configuration:");
    let effective_config = config.effective_config();
    for (key, (value, source)) in effective_config {
        println!("    {key} = {value} (from {source})");
    }

    // Check for partial artifacts and resume capabilities
    let phases = [
        PhaseId::Requirements,
        PhaseId::Design,
        PhaseId::Tasks,
        PhaseId::Review,
        PhaseId::Fixup,
        PhaseId::Final,
    ];
    let mut partial_phases = Vec::new();
    let mut completed_phases = Vec::new();

    for phase in phases {
        if handle.artifact_manager().has_partial_artifact(phase) {
            partial_phases.push(phase);
        }
        if handle.artifact_manager().phase_completed(phase) {
            completed_phases.push(phase);
        }
    }

    if !partial_phases.is_empty() {
        println!("\n  Partial artifacts found:");
        for phase in partial_phases {
            println!("    - {} (from failed execution)", phase.as_str());
        }
    }

    if !completed_phases.is_empty() {
        println!("\n  Completed phases:");
        for phase in completed_phases {
            println!("    - {}", phase.as_str());
        }
    }

    // Check for pending fixups and show intended targets (R5.6)
    check_and_display_fixup_targets(&handle, spec_id)?;

    // Show resume suggestions
    match latest_completed {
        Some(PhaseId::Requirements) => {
            println!("\n  Resume options:");
            println!("    - Continue to Design: xchecker resume {spec_id} --phase design");
            println!("    - Re-run Requirements: xchecker resume {spec_id} --phase requirements");
        }
        Some(PhaseId::Design) => {
            println!("\n  Resume options:");
            println!("    - Continue to Tasks: xchecker resume {spec_id} --phase tasks");
            println!("    - Re-run Design: xchecker resume {spec_id} --phase design");
        }
        Some(PhaseId::Tasks) => {
            println!("\n  Resume options:");
            println!("    - Continue to Review: xchecker resume {spec_id} --phase review");
            println!("    - Re-run Tasks: xchecker resume {spec_id} --phase tasks");
        }
        Some(PhaseId::Review) => {
            println!("\n  Resume options:");
            println!("    - Continue to Fixup: xchecker resume {spec_id} --phase fixup");
            println!("    - Re-run Review: xchecker resume {spec_id} --phase review");
        }
        Some(_) => {
            println!("\n  Resume options:");
            println!("    - Re-run any phase: xchecker resume {spec_id} --phase <phase_name>");
        }
        None => {
            println!("\n  Resume options:");
            println!(
                "    - Start from Requirements: xchecker resume {spec_id} --phase requirements"
            );
        }
    }

    Ok(())
}

/// Check for pending fixups and display intended targets (R5.6)
fn check_and_display_fixup_targets(handle: &OrchestratorHandle, spec_id: &str) -> Result<()> {
    use crate::fixup::{FixupMode, FixupParser};

    // Check if Review phase is completed and has fixup markers
    let base_path = handle.artifact_manager().base_path();
    let review_md_path = base_path.join("artifacts").join("30-review.md");

    if !review_md_path.exists() {
        return Ok(()); // No review phase completed yet
    }

    // Read the review content
    let review_content = match std::fs::read_to_string(&review_md_path) {
        Ok(content) => content,
        Err(_) => return Ok(()), // Can't read review file, skip fixup check
    };

    // Create fixup parser in preview mode to check for targets
    let fixup_parser = FixupParser::new(FixupMode::Preview, base_path.clone().into())?;

    // Check if there are fixup markers
    if !fixup_parser.has_fixup_markers(&review_content) {
        return Ok(()); // No fixups needed
    }

    // Parse diffs to get intended targets
    match fixup_parser.parse_diffs(&review_content) {
        Ok(diffs) => {
            if !diffs.is_empty() {
                println!("\n  Pending fixups detected:");
                println!("    Fixup markers found in review phase");
                println!("    Intended targets ({} files):", diffs.len());

                for diff in &diffs {
                    println!("      - {}", diff.target_file);
                }

                // Show preview information
                match fixup_parser.preview_changes(&diffs) {
                    Ok(preview) => {
                        if !preview.all_valid {
                            println!("    ⚠ Warning: Some diffs failed validation");
                        }

                        if !preview.warnings.is_empty() {
                            println!("    Validation warnings:");
                            for warning in &preview.warnings {
                                println!("      - {warning}");
                            }
                        }

                        // Show change summary
                        let mut total_added = 0;
                        let mut total_removed = 0;
                        for (file, summary) in &preview.change_summary {
                            total_added += summary.lines_added;
                            total_removed += summary.lines_removed;
                            if !summary.validation_passed {
                                println!("      ✗ {file}: validation failed");
                            }
                        }

                        if total_added > 0 || total_removed > 0 {
                            println!(
                                "    Estimated changes: +{total_added} lines, -{total_removed} lines"
                            );
                        }
                    }
                    Err(e) => {
                        println!("    ⚠ Warning: Failed to preview changes: {e}");
                    }
                }

                println!("\n    To apply fixups:");
                println!("      xchecker resume {spec_id} --phase fixup --apply-fixups");
                println!("    To preview only (default):");
                println!("      xchecker resume {spec_id} --phase fixup");
            }
        }
        Err(e) => {
            println!("\n  Fixup parsing error: {e}");
            println!("    Review phase contains fixup markers but diffs could not be parsed");
        }
    }

    Ok(())
}

/// Execute the resume command
#[allow(clippy::too_many_arguments)]
async fn execute_resume_command(
    spec_id: &str,
    phase_name: &str,
    dry_run: bool,
    verbose: bool,
    force: bool,
    apply_fixups: bool,
    strict_lock: bool,
    config: &Config,
    cli_args: &CliArgs,
    redactor: &Arc<SecretRedactor>,
) -> Result<()> {
    // Create logger for verbose output and timing (R7.5, NFR5)
    let mut logger = Logger::new(verbose);
    logger.start_timing("total_execution");

    // Parse phase name
    let phase_id = match phase_name.to_lowercase().as_str() {
        "requirements" => PhaseId::Requirements,
        "design" => PhaseId::Design,
        "tasks" => PhaseId::Tasks,
        "review" => PhaseId::Review,
        "fixup" => PhaseId::Fixup,
        "final" => PhaseId::Final,
        _ => {
            return Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "phase".to_string(),
                value: format!("Unknown phase '{phase_name}'. Valid phases: requirements, design, tasks, review, fixup, final"),
            }).into());
        }
    };

    logger.verbose(&format!(
        "Resuming spec {} from {} phase",
        spec_id,
        phase_id.as_str()
    ));
    if dry_run {
        logger.verbose("Running in dry-run mode (no Claude calls will be made)");
    }

    // Check for lockfile drift (R10.2, R10.4)
    let model_full_name = config.defaults.model.as_deref().unwrap_or("haiku");
    let claude_cli_version = detect_claude_cli_version().unwrap_or_else(|_| "unknown".to_string());
    let _lock_drift =
        check_lockfile_drift(spec_id, strict_lock, model_full_name, &claude_cli_version)?;

    // Configure execution using shared helper
    // Note: Problem statement is not passed for resume - it's already persisted in spec dir
    let orchestrator_config = build_orchestrator_config(
        dry_run,
        verbose,
        apply_fixups,
        config,
        cli_args,
        None,
        redactor.clone(),
    );

    // Create orchestrator handle (this will acquire the file lock)
    logger.start_timing("orchestrator_setup");
    let mut handle = OrchestratorHandle::with_config_and_force(spec_id, orchestrator_config, force)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;
    logger.end_timing("orchestrator_setup");

    // Check if spec exists
    let base_path = handle.artifact_manager().base_path();
    if !base_path.exists() {
        return Err(XCheckerError::Config(ConfigError::NotFound {
            path: format!("Spec directory: {base_path}"),
        })
        .into());
    }

    logger.verbose(&format!(
        "Checking dependencies for {} phase...",
        phase_id.as_str()
    ));

    // Execute resume
    logger.start_timing(&format!("{}_phase", phase_id.as_str()));
    let result = handle
        .run_phase(phase_id)
        .await
        .with_context(|| format!("Failed to resume {} phase", phase_id.as_str()))?;
    logger.end_timing(&format!("{}_phase", phase_id.as_str()));

    // Report results
    logger.end_timing("total_execution");

    if result.success {
        println!("✓ {} phase completed successfully", phase_id.as_str());

        logger.verbose(&format!("Phase: {}", result.phase.as_str()));
        logger.verbose(&format!("Exit code: {}", result.exit_code));
        logger.verbose(&format!(
            "Artifacts created: {}",
            result.artifact_paths.len()
        ));

        for (i, path) in result.artifact_paths.iter().enumerate() {
            logger.verbose(&format!("  {}: {}", i + 1, path.display()));
        }

        if let Some(receipt_path) = &result.receipt_path {
            logger.verbose(&format!("Receipt: {}", receipt_path.display()));
        }

        // Print performance summary if verbose (R7.5, NFR5)
        logger.print_performance_summary();

        // Show next steps based on completed phase
        println!("\nNext steps:");
        match phase_id {
            PhaseId::Requirements => {
                println!(
                    "  - Review the generated requirements in .xchecker/specs/{spec_id}/artifacts/"
                );
                println!("  - Continue to Design phase: xchecker resume {spec_id} --phase design");
            }
            PhaseId::Design => {
                println!("  - Review the generated design in .xchecker/specs/{spec_id}/artifacts/");
                println!("  - Continue to Tasks phase: xchecker resume {spec_id} --phase tasks");
            }
            PhaseId::Tasks => {
                println!("  - Review the generated tasks in .xchecker/specs/{spec_id}/artifacts/");
                println!("  - Continue to Review phase: xchecker resume {spec_id} --phase review");
            }
            _ => {
                println!("  - Check status with: xchecker status {spec_id}");
            }
        }
    } else {
        // Create structured error for phase failure (R1.3, R4.3)
        let phase_error = PhaseError::ExecutionFailed {
            phase: result.phase.as_str().to_string(),
            code: result.exit_code,
        };
        let xchecker_error = XCheckerError::Phase(phase_error);

        // Report with full context and suggestions
        let report = ErrorReport::new(&xchecker_error);
        eprintln!("{}", report.format_with_redactor(redactor.as_ref()));

        // Enhanced error reporting for phase failures (R1.3, R4.3)
        if let Some(error_msg) = &result.error {
            let redacted_error_msg = redactor.redact_string(error_msg);
            eprintln!("\n  Phase failure details: {redacted_error_msg}");
        }

        // Show partial artifacts location (R4.3)
        eprintln!("\n  Debugging information:");
        if !result.artifact_paths.is_empty() {
            eprintln!("    Partial artifacts:");
            for path in &result.artifact_paths {
                eprintln!("      - {}", path.display());
            }
        }
        eprintln!("    Spec directory: .xchecker/specs/{spec_id}/");

        if let Some(receipt_path) = &result.receipt_path {
            eprintln!("    Execution receipt: {}", receipt_path.display());
        }

        // Provide recovery suggestions
        eprintln!("\n  Recovery options:");
        eprintln!("    - Review partial outputs and receipt for error details");
        eprintln!("    - Fix any configuration or connectivity issues");
        eprintln!("    - Retry with: xchecker resume {spec_id} --phase {phase_name}");
        eprintln!(
            "    - Test configuration with: xchecker resume {spec_id} --phase {phase_name} --dry-run"
        );

        std::process::exit(result.exit_code);
    }

    Ok(())
}

/// Execute the clean command
fn execute_clean_command(spec_id: &str, hard: bool, force: bool, _config: &Config) -> Result<()> {
    use crate::lock::utils;

    // Check if clean operation is allowed (no active locks unless forced)
    if let Err(lock_error) = utils::can_clean(spec_id, force, None) {
        return Err(anyhow::anyhow!(
            "Cannot clean spec '{spec_id}': {lock_error}"
        ));
    }

    // Collect information we need before dropping the handle
    let (base_path, artifacts_path, receipts_path, context_path, artifacts, receipts) = {
        // Create handle to access managers (this will acquire a lock)
        let handle = OrchestratorHandle::with_force(spec_id, force)
            .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;

        // Check if spec directory exists
        let base_path = handle.artifact_manager().base_path();
        if !base_path.exists() {
            println!("No spec found for ID: {spec_id}");
            println!("Directory: {base_path} (does not exist)");
            return Ok(());
        }

        // Show what will be cleaned
        println!("Clean spec: {spec_id}");
        println!("  Directory: {base_path}");

        // List what will be removed
        let artifacts = handle
            .artifact_manager()
            .list_artifacts()
            .with_context(|| "Failed to list artifacts")?;
        let receipts = handle
            .receipt_manager()
            .list_receipts()
            .with_context(|| "Failed to list receipts")?;

        if artifacts.is_empty() && receipts.is_empty() {
            println!("  Nothing to clean (no artifacts or receipts found)");
            // Still need to remove the directory if --hard is specified
            if !hard {
                return Ok(());
            }
        }

        println!("  Will remove:");
        if !artifacts.is_empty() {
            println!("    Artifacts: {} files", artifacts.len());
            for artifact in &artifacts {
                println!("      - {artifact}");
            }
        }

        if !receipts.is_empty() {
            println!("    Receipts: {} files", receipts.len());
            for receipt in &receipts {
                let receipt_filename = format!(
                    "{}-{}.json",
                    receipt.phase,
                    receipt.emitted_at.format("%Y%m%d_%H%M%S")
                );
                println!("      - {receipt_filename}");
            }
        }

        // Get paths before dropping handle (clone to own the data)
        let artifacts_path = handle.artifact_manager().artifacts_path().to_path_buf();
        let receipts_path = base_path.join("receipts");
        let context_path = base_path.join("context");
        let base_path_owned = base_path.to_path_buf();

        (
            base_path_owned,
            artifacts_path,
            receipts_path,
            context_path,
            artifacts,
            receipts,
        )
        // Handle is dropped here, releasing the lock
    };

    // Confirmation prompt (R8.1)
    if !hard {
        println!("\nThis will permanently delete all artifacts and receipts for spec '{spec_id}'.");
        print!("Are you sure? (y/N): ");
        // Flush stdout, logging a warning if it fails (non-fatal)
        if let Err(e) = std::io::stdout().flush() {
            tracing::warn!("Failed to flush stdout: {}", e);
        }

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Clean cancelled.");
            return Ok(());
        }
    }

    // Perform cleanup (R8.2) - orchestrator lock is now released

    let mut removed_count = 0;

    // Remove artifacts directory
    if artifacts_path.exists() {
        std::fs::remove_dir_all(&artifacts_path)
            .with_context(|| format!("Failed to remove artifacts directory: {artifacts_path}"))?;
        removed_count += artifacts.len();
        println!("✓ Removed artifacts directory");
    }

    // Remove receipts directory
    if receipts_path.exists() {
        std::fs::remove_dir_all(&receipts_path)
            .with_context(|| format!("Failed to remove receipts directory: {receipts_path}"))?;
        removed_count += receipts.len();
        println!("✓ Removed receipts directory");
    }

    // Remove context directory
    if context_path.exists() {
        std::fs::remove_dir_all(&context_path)
            .with_context(|| format!("Failed to remove context directory: {context_path}"))?;
        println!("✓ Removed context directory");
    }

    // Remove the spec directory
    if base_path.exists() {
        if hard {
            // With --hard, remove the entire spec directory including any remaining files
            std::fs::remove_dir_all(&base_path)
                .with_context(|| format!("Failed to remove spec directory: {base_path}"))?;
            println!("✓ Removed spec directory completely");
        } else {
            // Without --hard, only remove if empty
            match std::fs::remove_dir(&base_path) {
                Ok(()) => {
                    println!("✓ Removed empty spec directory");
                }
                Err(_) => {
                    // Directory not empty, that's fine
                    println!("✓ Spec directory retained (contains other files)");
                }
            }
        }
    }

    println!("\nClean completed successfully.");
    println!("  Removed {removed_count} files total");

    Ok(())
}

/// Create default configuration from Config struct and CLI args
fn create_default_config(
    verbose: bool,
    config: &Config,
    cli_args: &CliArgs,
) -> HashMap<String, String> {
    let mut config_map = HashMap::new();

    if verbose {
        config_map.insert("verbose".to_string(), "true".to_string());
    }

    // Use values from the configuration system
    if let Some(packet_max_bytes) = config.defaults.packet_max_bytes {
        config_map.insert("packet_max_bytes".to_string(), packet_max_bytes.to_string());
    }

    if let Some(packet_max_lines) = config.defaults.packet_max_lines {
        config_map.insert("packet_max_lines".to_string(), packet_max_lines.to_string());
    }

    if let Some(max_turns) = config.defaults.max_turns {
        config_map.insert("max_turns".to_string(), max_turns.to_string());
    }

    if let Some(model) = &config.defaults.model {
        config_map.insert("model".to_string(), model.clone());
    }

    if let Some(output_format) = &config.defaults.output_format {
        config_map.insert("output_format".to_string(), output_format.clone());
    }

    if let Some(phase_timeout) = config.defaults.phase_timeout {
        config_map.insert("phase_timeout".to_string(), phase_timeout.to_string());
    }

    if let Some(stdout_cap_bytes) = config.defaults.stdout_cap_bytes {
        config_map.insert("stdout_cap_bytes".to_string(), stdout_cap_bytes.to_string());
    }

    if let Some(stderr_cap_bytes) = config.defaults.stderr_cap_bytes {
        config_map.insert("stderr_cap_bytes".to_string(), stderr_cap_bytes.to_string());
    }

    if let Some(lock_ttl_seconds) = config.defaults.lock_ttl_seconds {
        config_map.insert("lock_ttl_seconds".to_string(), lock_ttl_seconds.to_string());
    }

    if let Some(debug_packet) = config.defaults.debug_packet
        && debug_packet
    {
        config_map.insert("debug_packet".to_string(), "true".to_string());
    }

    if let Some(allow_links) = config.defaults.allow_links
        && allow_links
    {
        config_map.insert("allow_links".to_string(), "true".to_string());
    }

    if let Some(runner_mode) = &config.runner.mode {
        config_map.insert("runner_mode".to_string(), runner_mode.clone());
    }

    if let Some(runner_distro) = &config.runner.distro {
        config_map.insert("runner_distro".to_string(), runner_distro.clone());
    }

    if let Some(claude_path) = &config.runner.claude_path {
        config_map.insert("claude_path".to_string(), claude_path.clone());
    }

    if let Some(provider) = &config.llm.provider {
        config_map.insert("llm_provider".to_string(), provider.clone());
    }

    if let Some(fallback_provider) = &config.llm.fallback_provider {
        config_map.insert(
            "llm_fallback_provider".to_string(),
            fallback_provider.clone(),
        );
    }

    if let Some(execution_strategy) = &config.llm.execution_strategy {
        config_map.insert("execution_strategy".to_string(), execution_strategy.clone());
    }

    if let Some(prompt_template) = &config.llm.prompt_template {
        config_map.insert("prompt_template".to_string(), prompt_template.clone());
    }

    if let Some(claude_config) = &config.llm.claude
        && let Some(binary) = &claude_config.binary
    {
        config_map.insert("llm_claude_binary".to_string(), binary.clone());
    }

    if let Some(gemini_config) = &config.llm.gemini {
        if let Some(binary) = &gemini_config.binary {
            config_map.insert("llm_gemini_binary".to_string(), binary.clone());
        }
        if let Some(default_model) = &gemini_config.default_model {
            config_map.insert(
                "llm_gemini_default_model".to_string(),
                default_model.clone(),
            );
        }
    }

    // Add new CLI arguments (R7.2, R7.4, R9.2)
    if !cli_args.allow.is_empty() {
        config_map.insert("allowed_tools".to_string(), cli_args.allow.join(","));
    }

    if !cli_args.deny.is_empty() {
        config_map.insert("disallowed_tools".to_string(), cli_args.deny.join(","));
    }

    if cli_args.dangerously_skip_permissions {
        config_map.insert(
            "dangerously_skip_permissions".to_string(),
            "true".to_string(),
        );
    }

    if !cli_args.ignore_secret_pattern.is_empty() {
        config_map.insert(
            "ignore_secret_patterns".to_string(),
            cli_args.ignore_secret_pattern.join("|"),
        );
    }

    if !cli_args.extra_secret_pattern.is_empty() {
        config_map.insert(
            "extra_secret_patterns".to_string(),
            cli_args.extra_secret_pattern.join("|"),
        );
    }

    // Add debug_packet flag (FR-PKT-006, FR-PKT-007) for CLI-only overrides
    if cli_args.debug_packet {
        config_map.insert("debug_packet".to_string(), "true".to_string());
    }

    config_map
}

/// Build an OrchestratorConfig from CLI parameters.
///
/// This helper reduces duplication between execute_spec_command and execute_resume_command
/// by combining create_default_config with the common additional parameters.
///
/// # Arguments
/// * `dry_run` - Whether to run in simulation mode
/// * `verbose` - Enable verbose logging
/// * `apply_fixups` - Whether to apply fixups (true) or preview (false)
/// * `config` - The loaded xchecker configuration
/// * `cli_args` - CLI arguments passed by the user
/// * `problem_statement` - Optional problem statement to include in phase prompts
fn build_orchestrator_config(
    dry_run: bool,
    verbose: bool,
    apply_fixups: bool,
    config: &Config,
    cli_args: &CliArgs,
    problem_statement: Option<&str>,
    redactor: Arc<SecretRedactor>,
) -> OrchestratorConfig {
    let mut config_map = create_default_config(verbose, config, cli_args);
    config_map.insert("logger_enabled".to_string(), verbose.to_string());
    config_map.insert("apply_fixups".to_string(), apply_fixups.to_string());

    // Include problem statement in config for prompt construction (FR-PKT)
    if let Some(ps) = problem_statement {
        config_map.insert("problem_statement".to_string(), ps.to_string());
    }

    OrchestratorConfig {
        dry_run,
        config: config_map,
        full_config: Some(config.clone()),
        selectors: Some(config.selectors.clone()),
        strict_validation: config.strict_validation(),
        redactor,
        hooks: Some(config.hooks.clone()),
    }
}

/// Enhance error reporting for common failure scenarios
fn enhance_error_context(error: &anyhow::Error) -> Option<Vec<String>> {
    let error_str = error.to_string();

    if error_str.contains("Failed to create orchestrator") {
        Some(vec![
            "Check that the current directory is writable".to_string(),
            "Ensure sufficient disk space is available".to_string(),
            "Verify directory permissions".to_string(),
            "Try running from a different directory".to_string(),
        ])
    } else if error_str.contains("Failed to execute") {
        Some(vec![
            "Check the spec ID is valid and doesn't contain special characters".to_string(),
            "Verify Claude CLI is installed and accessible".to_string(),
            "Try running with --dry-run to test configuration".to_string(),
            "Check your internet connection if using Claude API".to_string(),
        ])
    } else if error_str.contains("Permission denied") {
        Some(vec![
            "Check file and directory permissions".to_string(),
            "Ensure you have write access to the current directory".to_string(),
            "Try running from your home directory or a writable location".to_string(),
        ])
    } else if error_str.contains("No such file or directory") {
        Some(vec![
            "Verify the specified paths exist".to_string(),
            "Check that you're running from the correct directory".to_string(),
            "Ensure all required files are present".to_string(),
        ])
    } else {
        None
    }
}

/// Execute the test command for integration validation
fn execute_test_command(components: bool, smoke: bool, verbose: bool) -> Result<()> {
    use crate::integration_tests;

    if verbose {
        println!("Running integration tests...");
    }

    // If no specific test type is specified, run both
    let run_components = components || !smoke;
    let run_smoke = smoke || !components;

    if run_components {
        integration_tests::validate_component_integration()
            .with_context(|| "Component integration validation failed")?;
    }

    if run_smoke {
        integration_tests::run_smoke_tests().with_context(|| "Smoke tests failed")?;
    }

    println!("✓ All integration tests passed successfully");
    Ok(())
}

/// Execute the benchmark command (NFR1 validation)
#[allow(clippy::too_many_arguments)]
fn execute_benchmark_command(
    file_count: usize,
    file_size: usize,
    iterations: usize,
    json: bool,
    max_empty_run_secs: Option<f64>,
    max_packetization_ms: Option<f64>,
    max_rss_mb: Option<f64>,
    max_commit_mb: Option<f64>,
    verbose: bool,
) -> Result<()> {
    use crate::benchmark::{BenchmarkConfig, BenchmarkRunner, BenchmarkThresholds};

    // Build custom thresholds if any overrides provided
    let mut thresholds = BenchmarkThresholds::default();
    if let Some(max_secs) = max_empty_run_secs {
        thresholds.empty_run_max_secs = max_secs;
    }
    if let Some(max_ms) = max_packetization_ms {
        thresholds.packetization_max_ms_per_100_files = max_ms;
    }
    if let Some(max_rss) = max_rss_mb {
        thresholds.max_rss_mb = Some(max_rss);
    }
    if let Some(max_commit) = max_commit_mb {
        thresholds.max_commit_mb = Some(max_commit);
    }

    // Only print header if not in JSON mode
    if !json {
        println!("=== xchecker Performance Benchmark ===");
        println!("Validating NFR1 performance targets:");
        println!("  - Empty run: ≤ {:.3}s", thresholds.empty_run_max_secs);
        println!(
            "  - Packetization: ≤ {:.1}ms per 100 files",
            thresholds.packetization_max_ms_per_100_files
        );
        if let Some(max_rss) = thresholds.max_rss_mb {
            println!("  - RSS memory: ≤ {max_rss:.1}MB");
        }
        if let Some(max_commit) = thresholds.max_commit_mb {
            println!("  - Commit memory: ≤ {max_commit:.1}MB");
        }
        println!();
    }

    // Create benchmark configuration
    let config = BenchmarkConfig {
        file_count,
        file_size_bytes: file_size,
        iterations,
        verbose: verbose && !json, // Suppress verbose output in JSON mode
        thresholds,
    };

    if verbose && !json {
        println!("Benchmark configuration:");
        println!("  File count: {}", config.file_count);
        println!("  File size: {} bytes", config.file_size_bytes);
        println!("  Iterations: {}", config.iterations);
        println!();
    }

    // Create and run benchmark
    let runner = BenchmarkRunner::new(config);
    let results = runner
        .run_all_benchmarks()
        .context("Failed to run benchmarks")?;

    // Output results
    if json {
        // Emit structured JSON output (FR-BENCH-004)
        // Use JCS canonicalization for consistent JSON output (FR-CLI-6)
        use serde_json::json;

        let json_output = json!({
            "ok": results.ok,
            "timings_ms": results.timings_ms,
            "rss_mb": results.rss_mb,
            "commit_mb": results.commit_mb,
            "violations": results.violations,
            "config": {
                "file_count": file_count,
                "file_size_bytes": file_size,
                "iterations": iterations,
            },
            "thresholds": {
                "empty_run_max_secs": runner.config.thresholds.empty_run_max_secs,
                "packetization_max_ms_per_100_files": runner.config.thresholds.packetization_max_ms_per_100_files,
                "max_rss_mb": runner.config.thresholds.max_rss_mb,
                "max_commit_mb": runner.config.thresholds.max_commit_mb,
            }
        });

        let canonical_json = emit_jcs(&json_output).context("Failed to emit benchmark JSON")?;
        println!("{canonical_json}");
    } else {
        // Print human-readable results
        runner.print_summary(&results);
    }

    // Exit with appropriate code based on results
    if results.ok {
        if !json {
            println!("\n✓ All performance targets met!");
        }
        Ok(())
    } else {
        if !json {
            println!("\n✗ Some performance targets not met.");
        }
        std::process::exit(1);
    }
}

/// Execute the doctor command for environment health checks
fn execute_doctor_command(json: bool, strict_exit: bool, config: &Config) -> Result<()> {
    use crate::doctor::DoctorCommand;

    // Create and run doctor command (wired through Doctor::run)
    let mut doctor = DoctorCommand::new(config.clone());
    let output = doctor
        .run_with_options_strict(strict_exit)
        .context("Failed to run doctor checks")?;

    if json {
        // Emit as canonical JSON (JCS) for stable diffs (FR-CLI-6)
        // Use emit_jcs for consistent canonicalization with receipts/status
        let json_output = emit_jcs(&output).context("Failed to emit doctor JSON")?;
        println!("{json_output}");
    } else {
        // Use log_doctor_report for human-readable output (wired into logging)
        crate::logging::log_doctor_report(&output);

        if !output.ok {
            println!();
            if strict_exit {
                println!(
                    "Some checks failed or warned (strict mode). Please address the issues above."
                );
            } else {
                println!(
                    "Some checks failed. Please address the issues above before using xchecker."
                );
            }
        }
    }

    // Exit with non-zero code if any check failed (R5.6)
    // In strict mode, warnings also cause non-zero exit
    if !output.ok {
        std::process::exit(1);
    }

    Ok(())
}

/// Execute the gate command for policy-based spec validation
/// Per FR-GATE (Requirements 4.5.1, 4.5.2, 4.5.3, 4.5.4)
fn execute_gate_command(
    spec_id: &str,
    policy_path: Option<&std::path::Path>,
    min_phase: Option<&str>,
    fail_on_pending_fixups: bool,
    max_phase_age: Option<&str>,
    json: bool,
) -> Result<()> {
    use crate::gate::{
        GateCommand, GatePolicy, emit_gate_json, load_policy_from_path, parse_duration,
        parse_phase, resolve_policy_path,
    };

    let policy_path = resolve_policy_path(policy_path).map_err(|e| {
        XCheckerError::Config(ConfigError::InvalidValue {
            key: "policy".to_string(),
            value: e.to_string(),
        })
    })?;

    let mut policy = if let Some(path) = policy_path {
        load_policy_from_path(&path).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "policy".to_string(),
                value: e.to_string(),
            })
        })?
    } else {
        GatePolicy::default()
    };

    if let Some(min_phase) = min_phase {
        policy.min_phase = parse_phase(min_phase).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "min_phase".to_string(),
                value: e.to_string(),
            })
        })?;
    }

    if fail_on_pending_fixups {
        policy.fail_on_pending_fixups = true;
    }

    if let Some(age_str) = max_phase_age {
        policy.max_phase_age = Some(parse_duration(age_str).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "max_phase_age".to_string(),
                value: e.to_string(),
            })
        })?);
    }

    // Execute gate evaluation
    let gate = GateCommand::new(spec_id.to_string(), policy);
    let result = gate
        .execute()
        .with_context(|| format!("Failed to evaluate gate for spec: {spec_id}"))?;

    // Output results
    if json {
        let json_output = emit_gate_json(&result).with_context(|| "Failed to emit gate JSON")?;
        println!("{json_output}");
    } else {
        // Human-friendly output
        if result.passed {
            println!("✓ {}", result.summary);
        } else {
            println!("✗ {}", result.summary);
        }

        println!();
        println!("Conditions evaluated:");
        for condition in &result.conditions {
            let status = if condition.passed { "✓" } else { "✗" };
            println!("  {} {}: {}", status, condition.name, condition.description);
            if let Some(actual) = &condition.actual {
                println!("      Actual: {}", actual);
            }
            if let Some(expected) = &condition.expected {
                println!("      Expected: {}", expected);
            }
        }

        if !result.failure_reasons.is_empty() {
            println!();
            println!("Failure reasons:");
            for reason in &result.failure_reasons {
                println!("  - {}", reason);
            }
        }
    }

    // Exit with appropriate code
    if result.passed {
        Ok(())
    } else {
        std::process::exit(crate::gate::exit_codes::POLICY_VIOLATION);
    }
}

/// Execute the init command to initialize a spec with optional lockfile
fn execute_init_command(spec_id: &str, create_lock: bool, config: &Config) -> Result<()> {
    use crate::lock::XCheckerLock;

    println!("Initializing spec: {spec_id}");

    // Create spec directory structure
    let spec_dir = PathBuf::from(".xchecker").join("specs").join(spec_id);
    let artifacts_dir = spec_dir.join("artifacts");
    let receipts_dir = spec_dir.join("receipts");
    let context_dir = spec_dir.join("context");

    // Check if spec already exists
    if spec_dir.exists() {
        println!("  Spec directory already exists: {}", spec_dir.display());

        // Check if lockfile exists
        let lock_path = spec_dir.join("lock.json");
        if lock_path.exists() {
            println!("  Lockfile already exists: {}", lock_path.display());

            if create_lock {
                println!("  ⚠ Warning: --create-lock specified but lockfile already exists");
                println!("  To update the lockfile, delete it first and run init again");
            }

            return Ok(());
        }
    } else {
        // Create directory structure (ignore benign races)
        crate::paths::ensure_dir_all(&artifacts_dir).with_context(|| {
            format!(
                "Failed to create artifacts directory: {}",
                artifacts_dir.display()
            )
        })?;
        crate::paths::ensure_dir_all(&receipts_dir).with_context(|| {
            format!(
                "Failed to create receipts directory: {}",
                receipts_dir.display()
            )
        })?;
        crate::paths::ensure_dir_all(&context_dir).with_context(|| {
            format!(
                "Failed to create context directory: {}",
                context_dir.display()
            )
        })?;

        println!("  ✓ Created spec directory: {}", spec_dir.display());
        println!("  ✓ Created artifacts directory");
        println!("  ✓ Created receipts directory");
        println!("  ✓ Created context directory");
    }

    // Create lockfile if requested
    if create_lock {
        // Get model from config or use default
        let model = config.defaults.model.as_deref().unwrap_or("haiku");

        // Get Claude CLI version (we'll need to detect this - for now use a placeholder)
        // In a real implementation, this would call `claude --version` and parse the output
        let claude_cli_version =
            detect_claude_cli_version().unwrap_or_else(|_| "unknown".to_string());

        let lock = XCheckerLock::new(model.to_string(), claude_cli_version.clone());

        lock.save(spec_id)
            .with_context(|| "Failed to save lockfile")?;

        println!("  ✓ Created lockfile: lock.json");
        println!("    Model: {model}");
        println!("    Claude CLI version: {claude_cli_version}");
        println!("    Schema version: 1");

        println!("\n  Lockfile will track drift for:");
        println!("    - Model changes (current: {model})");
        println!("    - Claude CLI version changes (current: {claude_cli_version})");
        println!("    - Schema version changes (current: 1)");
        println!("\n  Use --strict-lock flag to hard fail on drift detection");
    } else {
        println!("\n  No lockfile created (use --create-lock to pin model and CLI version)");
    }

    println!("\nSpec '{spec_id}' initialized successfully");
    println!("  Directory: {}", spec_dir.display());

    Ok(())
}

/// Detect Claude CLI version by running `claude --version`
fn detect_claude_cli_version() -> Result<String> {
    use crate::runner::CommandSpec;

    let output = CommandSpec::new("claude")
        .arg("--version")
        .to_command()
        .output()
        .context("Failed to execute 'claude --version'")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "claude --version exited with non-zero status"
        ));
    }

    let version_str = String::from_utf8(output.stdout)
        .context("Failed to parse claude --version output as UTF-8")?;

    // Parse version from output (format: "claude 0.8.1" or similar)
    let version = version_str
        .split_whitespace()
        .last()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse version from output"))?
        .to_string();

    Ok(version)
}

/// Check for lockfile drift and warn or fail based on `strict_lock` flag
fn check_lockfile_drift(
    spec_id: &str,
    strict_lock: bool,
    model_full_name: &str,
    claude_cli_version: &str,
) -> Result<Option<crate::types::LockDrift>> {
    use crate::lock::{RunContext, XCheckerLock};

    // Try to load lockfile
    let lock = match XCheckerLock::load(spec_id) {
        Ok(Some(lock)) => lock,
        Ok(None) => return Ok(None), // No lockfile, no drift
        Err(e) => {
            eprintln!("⚠ Warning: Failed to load lockfile: {e}");
            return Ok(None);
        }
    };

    // Create current run context
    let context = RunContext {
        model_full_name: model_full_name.to_string(),
        claude_cli_version: claude_cli_version.to_string(),
        schema_version: "1".to_string(),
    };

    // Detect drift
    if let Some(drift) = lock.detect_drift(&context) {
        // Print drift warning
        eprintln!("\n⚠ Lockfile drift detected for spec '{spec_id}':");

        if let Some(ref model_drift) = drift.model_full_name {
            eprintln!("  Model: {} → {}", model_drift.locked, model_drift.current);
        }

        if let Some(ref cli_drift) = drift.claude_cli_version {
            eprintln!("  Claude CLI: {} → {}", cli_drift.locked, cli_drift.current);
        }

        if let Some(ref schema_drift) = drift.schema_version {
            eprintln!(
                "  Schema: {} → {}",
                schema_drift.locked, schema_drift.current
            );
        }

        if strict_lock {
            eprintln!("\n✗ Strict lock mode enabled: failing due to drift");
            eprintln!("  To proceed, either:");
            eprintln!(
                "    - Update the lockfile: rm .xchecker/specs/{spec_id}/lock.json && xchecker init {spec_id} --create-lock"
            );
            eprintln!("    - Remove --strict-lock flag to allow drift with warning");

            return Err(anyhow::anyhow!("Lockfile drift detected in strict mode"));
        }
        eprintln!("\n  Continuing with drift (use --strict-lock to fail on drift)");

        Ok(Some(drift))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)] // Test helper functions defined after tests is intentional
#[allow(clippy::await_holding_lock)] // Test synchronization using mutex guards across awaits is intentional
mod tests {
    use super::*;
    use std::env;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    // Global lock for tests that mutate process-global CLI state (env vars, cwd).
    // Any test that uses `TestEnvGuard` or `cli_env_guard()` will be serialized.
    static CLI_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn cli_env_guard() -> MutexGuard<'static, ()> {
        CLI_ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    struct TestEnvGuard {
        // Hold the lock for the entire lifetime of the guard
        _lock: MutexGuard<'static, ()>,
        _temp_dir: TempDir,
        original_dir: PathBuf,
        original_xchecker_home: Option<String>,
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            // Restore env and cwd while still holding the lock
            match &self.original_xchecker_home {
                Some(val) => unsafe { env::set_var("XCHECKER_HOME", val) },
                None => unsafe { env::remove_var("XCHECKER_HOME") },
            }
            let _ = env::set_current_dir(&self.original_dir);
            // _lock field drops last, releasing the mutex
        }
    }

    fn setup_test_environment() -> TestEnvGuard {
        // Take the global CLI lock first
        let lock = cli_env_guard();

        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        let original_xchecker_home = env::var("XCHECKER_HOME").ok();

        // From here onwards we're serialized against other CLI tests
        env::set_current_dir(temp_dir.path()).unwrap();

        TestEnvGuard {
            _lock: lock,
            _temp_dir: temp_dir,
            original_dir,
            original_xchecker_home,
        }
    }

    #[test]
    fn test_create_default_config() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let config_map = create_default_config(true, &config, &cli_args);

        assert_eq!(config_map.get("verbose"), Some(&"true".to_string()));
        assert_eq!(
            config_map.get("packet_max_bytes"),
            Some(&"65536".to_string())
        );
        assert_eq!(
            config_map.get("packet_max_lines"),
            Some(&"1200".to_string())
        );
    }

    #[test]
    fn test_create_default_config_no_verbose() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let config_map = create_default_config(false, &config, &cli_args);

        assert!(!config_map.contains_key("verbose"));
        assert_eq!(
            config_map.get("packet_max_bytes"),
            Some(&"65536".to_string())
        );
        assert_eq!(
            config_map.get("packet_max_lines"),
            Some(&"1200".to_string())
        );
    }

    #[tokio::test]
    async fn test_spec_command_execution() -> anyhow::Result<()> {
        use tempfile::TempDir;

        // Take the global CLI lock for env/cwd mutations
        let _lock = cli_env_guard();

        // Save original state
        let original_dir = std::env::current_dir()?;
        let original_xchecker_home = std::env::var("XCHECKER_HOME").ok();
        let original_skip_llm = std::env::var("XCHECKER_SKIP_LLM_TESTS").ok();

        // Setup isolated test root
        let temp = TempDir::new()?;
        let root = temp.path();

        // Make it look like a repo root
        std::fs::create_dir_all(root.join(".git"))?;

        // Set process environment
        std::env::set_current_dir(root)?;
        unsafe {
            std::env::set_var("XCHECKER_HOME", root);
            std::env::set_var("XCHECKER_SKIP_LLM_TESTS", "1");
        }

        // Create minimal config
        std::fs::write(
            root.join("xchecker.toml"),
            r#"
[runner]
runner_mode = "native"

[packet]
packet_max_bytes = 1048576
packet_max_lines = 5000
"#,
        )?;

        // Create minimal CLI args for dry-run
        let cli_args = CliArgs::default();

        let config = Config::discover(&cli_args)?;
        let redactor = Arc::new(SecretRedactor::from_config(&config)?);

        // Create a minimal input file for the spec
        std::fs::write(root.join("input.txt"), "Test requirement")?;

        // Test dry-run execution (fast, no real LLMs)
        // This should complete quickly and not hang
        let result = execute_spec_command(
            "test-spec",
            "fs",
            Some("input.txt"),
            Some(root.to_str().unwrap()), // repo path
            true,                         // dry_run = true
            false,
            false,
            false,
            false,
            &config,
            &cli_args,
            &redactor,
        )
        .await;

        // Restore original environment before asserting
        let _ = std::env::set_current_dir(&original_dir);
        match original_xchecker_home {
            Some(val) => unsafe { std::env::set_var("XCHECKER_HOME", val) },
            None => unsafe { std::env::remove_var("XCHECKER_HOME") },
        }
        match original_skip_llm {
            Some(val) => unsafe { std::env::set_var("XCHECKER_SKIP_LLM_TESTS", val) },
            None => unsafe { std::env::remove_var("XCHECKER_SKIP_LLM_TESTS") },
        }

        // In dry-run mode with a valid source, this should succeed
        // The important thing is it doesn't hang and completes quickly
        assert!(
            result.is_ok(),
            "Dry-run spec execution should succeed: {:?}",
            result.err()
        );

        Ok(())
    }

    #[test]
    fn test_status_command_no_spec() {
        let _temp_dir = setup_test_environment();

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();

        // Test status for non-existent spec
        let result = execute_status_command("nonexistent-spec", false, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_command_with_spec() {
        let _temp_dir = setup_test_environment();

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();

        // Note: We can't easily test spec creation with stdin in unit tests
        // This test just verifies status command works with non-existent spec
        let result = execute_status_command("test-status-spec", false, &config);
        assert!(result.is_ok());
    }

    // ===== Spec JSON Output Tests (Task 21.1) =====
    // **Property: JSON output includes schema version**
    // **Validates: Requirements 4.1.1**

    #[test]
    fn test_spec_json_output_schema_version() {
        // Test that spec JSON output includes schema_version field
        use crate::types::{PhaseInfo, SpecConfigSummary, SpecOutput};

        let output = SpecOutput {
            schema_version: "spec-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phases: vec![PhaseInfo {
                phase_id: "requirements".to_string(),
                status: "completed".to_string(),
                last_run: Some(chrono::Utc::now()),
            }],
            config_summary: SpecConfigSummary {
                execution_strategy: "controlled".to_string(),
                provider: Some("claude-cli".to_string()),
                spec_path: ".xchecker/specs/test-spec".to_string(),
            },
        };

        // Emit as JSON
        let json_result = emit_spec_json(&output);
        assert!(json_result.is_ok(), "Failed to emit spec JSON");

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify schema_version is present and correct
        assert_eq!(parsed["schema_version"], "spec-json.v1");
        assert_eq!(parsed["spec_id"], "test-spec");
    }

    #[test]
    fn test_spec_json_output_excludes_packet_contents() {
        // Test that spec JSON output excludes full packet contents
        // per Requirements 4.1.4
        use crate::types::{PhaseInfo, SpecConfigSummary, SpecOutput};

        let output = SpecOutput {
            schema_version: "spec-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phases: vec![
                PhaseInfo {
                    phase_id: "requirements".to_string(),
                    status: "completed".to_string(),
                    last_run: None,
                },
                PhaseInfo {
                    phase_id: "design".to_string(),
                    status: "not_started".to_string(),
                    last_run: None,
                },
            ],
            config_summary: SpecConfigSummary {
                execution_strategy: "controlled".to_string(),
                provider: None,
                spec_path: ".xchecker/specs/test-spec".to_string(),
            },
        };

        let json_result = emit_spec_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify no packet contents are present
        assert!(
            parsed.get("packet").is_none(),
            "JSON should not contain packet field"
        );
        assert!(
            parsed.get("artifacts").is_none(),
            "JSON should not contain artifacts field"
        );
        assert!(
            parsed.get("raw_response").is_none(),
            "JSON should not contain raw_response field"
        );

        // Verify only expected fields are present
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("spec_id").is_some());
        assert!(parsed.get("phases").is_some());
        assert!(parsed.get("config_summary").is_some());
    }

    #[test]
    fn test_spec_json_command_no_spec() {
        let _temp_dir = setup_test_environment();

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();

        // Test spec --json for non-existent spec
        let result = execute_spec_json_command("nonexistent-spec-json", &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_spec_json_canonical_format() {
        // Test that spec JSON output is in canonical JCS format (no extra whitespace)
        use crate::types::{PhaseInfo, SpecConfigSummary, SpecOutput};

        let output = SpecOutput {
            schema_version: "spec-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phases: vec![PhaseInfo {
                phase_id: "requirements".to_string(),
                status: "completed".to_string(),
                last_run: None,
            }],
            config_summary: SpecConfigSummary {
                execution_strategy: "controlled".to_string(),
                provider: None,
                spec_path: ".xchecker/specs/test-spec".to_string(),
            },
        };

        let json_result = emit_spec_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();

        // Verify canonical JSON properties (no extra whitespace, no newlines)
        assert!(
            !json_str.contains("  "),
            "Canonical JSON should not have indentation"
        );
        assert!(
            !json_str.contains('\n'),
            "Canonical JSON should not have newlines"
        );
    }

    #[test]
    fn test_spec_json_all_phases_present() {
        // Test that all phases are represented in the output
        use crate::types::{PhaseInfo, SpecConfigSummary, SpecOutput};

        let phases = vec![
            PhaseInfo {
                phase_id: "requirements".to_string(),
                status: "completed".to_string(),
                last_run: None,
            },
            PhaseInfo {
                phase_id: "design".to_string(),
                status: "pending".to_string(),
                last_run: None,
            },
            PhaseInfo {
                phase_id: "tasks".to_string(),
                status: "not_started".to_string(),
                last_run: None,
            },
            PhaseInfo {
                phase_id: "review".to_string(),
                status: "not_started".to_string(),
                last_run: None,
            },
            PhaseInfo {
                phase_id: "fixup".to_string(),
                status: "not_started".to_string(),
                last_run: None,
            },
            PhaseInfo {
                phase_id: "final".to_string(),
                status: "not_started".to_string(),
                last_run: None,
            },
        ];

        let output = SpecOutput {
            schema_version: "spec-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phases,
            config_summary: SpecConfigSummary {
                execution_strategy: "controlled".to_string(),
                provider: Some("openrouter".to_string()),
                spec_path: ".xchecker/specs/test-spec".to_string(),
            },
        };

        let json_result = emit_spec_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all 6 phases are present
        let phases_array = parsed["phases"].as_array().unwrap();
        assert_eq!(phases_array.len(), 6);

        // Verify phase IDs
        let phase_ids: Vec<&str> = phases_array
            .iter()
            .map(|p| p["phase_id"].as_str().unwrap())
            .collect();
        assert!(phase_ids.contains(&"requirements"));
        assert!(phase_ids.contains(&"design"));
        assert!(phase_ids.contains(&"tasks"));
        assert!(phase_ids.contains(&"review"));
        assert!(phase_ids.contains(&"fixup"));
        assert!(phase_ids.contains(&"final"));
    }

    #[test]
    fn test_benchmark_command_basic() {
        // Test basic benchmark execution with realistic thresholds for test environments
        // Use more generous thresholds since test environments can be slower
        let result = execute_benchmark_command(
            5,           // file_count
            100,         // file_size
            2,           // iterations
            false,       // json
            Some(10.0),  // max_empty_run_secs - generous for test env
            Some(500.0), // max_packetization_ms - generous for test env (25ms for 5 files)
            None,        // max_rss_mb
            None,        // max_commit_mb
            false,       // verbose
        );

        // Should succeed with realistic test environment thresholds
        assert!(result.is_ok());
    }

    #[test]
    fn test_benchmark_command_with_threshold_overrides() {
        // Test benchmark with custom thresholds (very generous to ensure pass)
        let result = execute_benchmark_command(
            5,             // file_count
            100,           // file_size
            2,             // iterations
            false,         // json
            Some(100.0),   // max_empty_run_secs - very generous
            Some(10000.0), // max_packetization_ms - very generous
            Some(1000.0),  // max_rss_mb - very generous
            Some(2000.0),  // max_commit_mb - very generous
            false,         // verbose
        );

        // Should succeed with generous thresholds
        assert!(result.is_ok());
    }

    #[test]
    fn test_benchmark_command_json_output() {
        // Test that JSON mode runs successfully
        // (We can't easily capture stdout in unit tests, but integration tests verify JSON structure)
        let result = execute_benchmark_command(
            5,             // file_count
            100,           // file_size
            2,             // iterations
            true,          // json - this is what we're testing
            Some(100.0),   // max_empty_run_secs
            Some(10000.0), // max_packetization_ms
            None,          // max_rss_mb
            None,          // max_commit_mb
            false,         // verbose (should be suppressed in JSON mode)
        );

        // Should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_benchmark_thresholds_applied() {
        // Test that custom thresholds are properly applied
        use crate::benchmark::{BenchmarkConfig, BenchmarkThresholds};

        let thresholds = BenchmarkThresholds {
            empty_run_max_secs: 3.0,
            packetization_max_ms_per_100_files: 150.0,
            max_rss_mb: Some(500.0),
            max_commit_mb: Some(1000.0),
        };

        let config = BenchmarkConfig {
            file_count: 10,
            file_size_bytes: 100,
            iterations: 2,
            verbose: false,
            thresholds,
        };

        // Verify thresholds are set correctly
        assert_eq!(config.thresholds.empty_run_max_secs, 3.0);
        assert_eq!(config.thresholds.packetization_max_ms_per_100_files, 150.0);
        assert_eq!(config.thresholds.max_rss_mb, Some(500.0));
        assert_eq!(config.thresholds.max_commit_mb, Some(1000.0));
    }

    #[test]
    fn test_benchmark_cli_parsing() {
        // Test that CLI arguments are properly parsed
        use clap::Parser;

        // Test basic benchmark command
        let args = vec![
            "xchecker",
            "benchmark",
            "--file-count",
            "50",
            "--iterations",
            "3",
        ];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Benchmark {
                    file_count,
                    iterations,
                    ..
                } => {
                    assert_eq!(file_count, 50);
                    assert_eq!(iterations, 3);
                }
                _ => panic!("Expected Benchmark command"),
            }
        }

        // Test benchmark with threshold overrides
        let args_with_thresholds = vec![
            "xchecker",
            "benchmark",
            "--max-empty-run-secs",
            "3.5",
            "--max-packetization-ms",
            "180.0",
            "--json",
        ];
        let cli_thresholds = Cli::try_parse_from(args_with_thresholds);
        assert!(cli_thresholds.is_ok());

        if let Ok(cli) = cli_thresholds {
            match cli.command {
                Commands::Benchmark {
                    max_empty_run_secs,
                    max_packetization_ms,
                    json,
                    ..
                } => {
                    assert_eq!(max_empty_run_secs, Some(3.5));
                    assert_eq!(max_packetization_ms, Some(180.0));
                    assert!(json);
                }
                _ => panic!("Expected Benchmark command"),
            }
        }
    }

    #[test]
    fn test_benchmark_default_values() {
        // Test that default values are applied correctly
        use clap::Parser;

        let args = vec!["xchecker", "benchmark"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Benchmark {
                    file_count,
                    file_size,
                    iterations,
                    json,
                    max_empty_run_secs,
                    max_packetization_ms,
                    ..
                } => {
                    assert_eq!(file_count, 100); // default
                    assert_eq!(file_size, 1024); // default
                    assert_eq!(iterations, 5); // default
                    assert!(!json); // default false
                    assert_eq!(max_empty_run_secs, None); // default None
                    assert_eq!(max_packetization_ms, None); // default None
                }
                _ => panic!("Expected Benchmark command"),
            }
        }
    }

    // ===== Status JSON Output Tests (Task 22) =====
    // **Property: JSON output includes schema version**
    // **Validates: Requirements 4.1.2**

    #[test]
    fn test_status_json_output_schema_version() {
        // Test that status JSON output includes schema_version field
        use crate::types::{PhaseStatusInfo, StatusJsonOutput};

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: "test-spec".to_string(),
            phase_statuses: vec![PhaseStatusInfo {
                phase_id: "requirements".to_string(),
                status: "success".to_string(),
                receipt_id: Some("requirements-20241201_100000".to_string()),
            }],
            pending_fixups: 0,
            has_errors: false,
            strict_validation: false,
            artifacts: Vec::new(),
            effective_config: std::collections::BTreeMap::new(),
            lock_drift: None,
        };

        // Emit as JSON
        let json_result = emit_status_json(&output);
        assert!(json_result.is_ok(), "Failed to emit status JSON");

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify schema_version is present and correct
        assert_eq!(parsed["schema_version"], "status-json.v2");
        assert_eq!(parsed["spec_id"], "test-spec");
    }

    #[test]
    fn test_status_json_output_has_required_fields() {
        // Test that status JSON output has all required fields per Requirements 4.1.2
        use crate::types::{PhaseStatusInfo, StatusJsonOutput};

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: "test-spec".to_string(),
            phase_statuses: vec![
                PhaseStatusInfo {
                    phase_id: "requirements".to_string(),
                    status: "success".to_string(),
                    receipt_id: Some("requirements-20241201_100000".to_string()),
                },
                PhaseStatusInfo {
                    phase_id: "design".to_string(),
                    status: "failed".to_string(),
                    receipt_id: Some("design-20241201_110000".to_string()),
                },
                PhaseStatusInfo {
                    phase_id: "tasks".to_string(),
                    status: "not_started".to_string(),
                    receipt_id: None,
                },
            ],
            pending_fixups: 3,
            has_errors: true,
            strict_validation: false,
            artifacts: Vec::new(),
            effective_config: std::collections::BTreeMap::new(),
            lock_drift: None,
        };

        let json_result = emit_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all required fields are present
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("spec_id").is_some());
        assert!(parsed.get("phase_statuses").is_some());
        assert!(parsed.get("pending_fixups").is_some());
        assert!(parsed.get("has_errors").is_some());
        assert!(
            parsed.get("strict_validation").is_some(),
            "strict_validation field should be present"
        );

        // Verify values
        assert_eq!(parsed["pending_fixups"], 3);
        assert_eq!(parsed["has_errors"], true);
        assert_eq!(parsed["strict_validation"], false);
    }

    #[test]
    fn test_status_json_canonical_format() {
        // Test that status JSON output is in canonical JCS format (no extra whitespace)
        use crate::types::{PhaseStatusInfo, StatusJsonOutput};

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: "test-spec".to_string(),
            phase_statuses: vec![PhaseStatusInfo {
                phase_id: "requirements".to_string(),
                status: "success".to_string(),
                receipt_id: None,
            }],
            pending_fixups: 0,
            has_errors: false,
            strict_validation: false,
            artifacts: Vec::new(),
            effective_config: std::collections::BTreeMap::new(),
            lock_drift: None,
        };

        let json_result = emit_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();

        // Verify canonical JSON properties (no extra whitespace, no newlines)
        assert!(
            !json_str.contains("  "),
            "Canonical JSON should not have indentation"
        );
        assert!(
            !json_str.contains('\n'),
            "Canonical JSON should not have newlines"
        );
    }

    #[test]
    fn test_status_json_excludes_raw_packet_contents() {
        // Test that status JSON output excludes raw packet contents (like raw_response)
        // but does include summarized artifacts and effective_config per v2 schema
        use crate::types::{
            ArtifactInfo, ConfigSource, ConfigValue, PhaseStatusInfo, StatusJsonOutput,
        };

        let mut effective_config = std::collections::BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ConfigValue {
                value: serde_json::Value::String("haiku".to_string()),
                source: ConfigSource::Config,
            },
        );

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: "test-spec".to_string(),
            phase_statuses: vec![PhaseStatusInfo {
                phase_id: "requirements".to_string(),
                status: "success".to_string(),
                receipt_id: Some("requirements-20241201_100000".to_string()),
            }],
            pending_fixups: 0,
            has_errors: false,
            strict_validation: false,
            artifacts: vec![ArtifactInfo {
                path: "artifacts/requirements.yaml".to_string(),
                blake3_first8: "abc12345".to_string(),
            }],
            effective_config,
            lock_drift: None,
        };

        let json_result = emit_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify no raw packet/response contents are present
        assert!(
            parsed.get("packet").is_none(),
            "JSON should not contain packet field"
        );
        assert!(
            parsed.get("raw_response").is_none(),
            "JSON should not contain raw_response field"
        );

        // Verify artifacts and effective_config ARE present in v2
        assert!(
            parsed.get("artifacts").is_some(),
            "JSON should contain artifacts field in v2"
        );
        assert!(
            parsed.get("effective_config").is_some(),
            "JSON should contain effective_config field in v2"
        );

        // Verify artifacts have only summary data (blake3_first8), not full content
        let artifacts = parsed["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0]["blake3_first8"], "abc12345");
        assert!(
            artifacts[0].get("content").is_none(),
            "Artifacts should not include full content"
        );
    }

    #[test]
    fn test_status_json_command_no_spec() {
        let _temp_dir = setup_test_environment();

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();

        // Test status --json for non-existent spec
        let result = execute_status_command("nonexistent-spec-json", true, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_json_all_phases_present() {
        // Test that all phases can be represented in the output
        use crate::types::{PhaseStatusInfo, StatusJsonOutput};

        let phase_statuses = vec![
            PhaseStatusInfo {
                phase_id: "requirements".to_string(),
                status: "success".to_string(),
                receipt_id: Some("requirements-20241201_100000".to_string()),
            },
            PhaseStatusInfo {
                phase_id: "design".to_string(),
                status: "success".to_string(),
                receipt_id: Some("design-20241201_110000".to_string()),
            },
            PhaseStatusInfo {
                phase_id: "tasks".to_string(),
                status: "failed".to_string(),
                receipt_id: Some("tasks-20241201_120000".to_string()),
            },
            PhaseStatusInfo {
                phase_id: "review".to_string(),
                status: "not_started".to_string(),
                receipt_id: None,
            },
            PhaseStatusInfo {
                phase_id: "fixup".to_string(),
                status: "not_started".to_string(),
                receipt_id: None,
            },
            PhaseStatusInfo {
                phase_id: "final".to_string(),
                status: "not_started".to_string(),
                receipt_id: None,
            },
        ];

        let output = StatusJsonOutput {
            schema_version: "status-json.v2".to_string(),
            spec_id: "test-spec".to_string(),
            phase_statuses,
            pending_fixups: 0,
            has_errors: true, // tasks failed
            strict_validation: false,
            artifacts: Vec::new(),
            effective_config: std::collections::BTreeMap::new(),
            lock_drift: None,
        };

        let json_result = emit_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all 6 phases are present
        let phases_array = parsed["phase_statuses"].as_array().unwrap();
        assert_eq!(phases_array.len(), 6);

        // Verify phase IDs
        let phase_ids: Vec<&str> = phases_array
            .iter()
            .map(|p| p["phase_id"].as_str().unwrap())
            .collect();
        assert!(phase_ids.contains(&"requirements"));
        assert!(phase_ids.contains(&"design"));
        assert!(phase_ids.contains(&"tasks"));
        assert!(phase_ids.contains(&"review"));
        assert!(phase_ids.contains(&"fixup"));
        assert!(phase_ids.contains(&"final"));
    }

    // ===== Resume JSON Output Tests (Task 23) =====
    // **Property: JSON output includes schema version**
    // **Validates: Requirements 4.1.3**

    #[test]
    fn test_resume_json_output_schema_version() {
        // Test that resume JSON output includes schema_version field
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let output = ResumeJsonOutput {
            schema_version: "resume-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phase: "design".to_string(),
            current_inputs: CurrentInputs {
                available_artifacts: vec!["00-requirements.md".to_string()],
                spec_exists: true,
                latest_completed_phase: Some("requirements".to_string()),
            },
            next_steps: "Run design phase to generate architecture and design from requirements."
                .to_string(),
        };

        // Emit as JSON
        let json_result = emit_resume_json(&output);
        assert!(json_result.is_ok(), "Failed to emit resume JSON");

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify schema_version is present and correct
        assert_eq!(parsed["schema_version"], "resume-json.v1");
        assert_eq!(parsed["spec_id"], "test-spec");
        assert_eq!(parsed["phase"], "design");
    }

    #[test]
    fn test_resume_json_output_has_required_fields() {
        // Test that resume JSON output has all required fields per Requirements 4.1.3
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let output = ResumeJsonOutput {
            schema_version: "resume-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phase: "tasks".to_string(),
            current_inputs: CurrentInputs {
                available_artifacts: vec![
                    "00-requirements.md".to_string(),
                    "10-design.md".to_string(),
                ],
                spec_exists: true,
                latest_completed_phase: Some("design".to_string()),
            },
            next_steps: "Run tasks phase to generate implementation tasks from design.".to_string(),
        };

        let json_result = emit_resume_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all required fields are present
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("spec_id").is_some());
        assert!(parsed.get("phase").is_some());
        assert!(parsed.get("current_inputs").is_some());
        assert!(parsed.get("next_steps").is_some());

        // Verify current_inputs structure
        let current_inputs = &parsed["current_inputs"];
        assert!(current_inputs.get("available_artifacts").is_some());
        assert!(current_inputs.get("spec_exists").is_some());
        assert!(current_inputs.get("latest_completed_phase").is_some());
    }

    #[test]
    fn test_resume_json_canonical_format() {
        // Test that resume JSON output is in canonical JCS format (no extra whitespace)
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let output = ResumeJsonOutput {
            schema_version: "resume-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phase: "requirements".to_string(),
            current_inputs: CurrentInputs {
                available_artifacts: vec![],
                spec_exists: true,
                latest_completed_phase: None,
            },
            next_steps: "Run requirements phase.".to_string(),
        };

        let json_result = emit_resume_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();

        // Verify canonical JSON properties (no extra whitespace, no newlines)
        assert!(
            !json_str.contains("  "),
            "Canonical JSON should not have indentation"
        );
        assert!(
            !json_str.contains('\n'),
            "Canonical JSON should not have newlines"
        );
    }

    #[test]
    fn test_resume_json_excludes_raw_artifacts() {
        // Test that resume JSON output excludes full packet and raw artifacts
        // per Requirements 4.1.4
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let output = ResumeJsonOutput {
            schema_version: "resume-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            phase: "design".to_string(),
            current_inputs: CurrentInputs {
                available_artifacts: vec!["00-requirements.md".to_string()],
                spec_exists: true,
                latest_completed_phase: Some("requirements".to_string()),
            },
            next_steps: "Run design phase.".to_string(),
        };

        let json_result = emit_resume_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify no packet contents or raw artifacts are present
        assert!(
            parsed.get("packet").is_none(),
            "JSON should not contain packet field"
        );
        assert!(
            parsed.get("artifacts").is_none(),
            "JSON should not contain artifacts field"
        );
        assert!(
            parsed.get("raw_response").is_none(),
            "JSON should not contain raw_response field"
        );
        assert!(
            parsed.get("artifact_contents").is_none(),
            "JSON should not contain artifact_contents field"
        );

        // Verify only artifact names are present, not contents
        let artifacts = parsed["current_inputs"]["available_artifacts"]
            .as_array()
            .unwrap();
        for artifact in artifacts {
            // Each artifact should be a simple string (name), not an object with contents
            assert!(
                artifact.is_string(),
                "Artifacts should be names only, not objects with contents"
            );
        }
    }

    #[test]
    fn test_resume_json_command_no_spec() {
        let _temp_dir = setup_test_environment();

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();

        // Test resume --json for non-existent spec
        let result = execute_resume_json_command("nonexistent-spec-json", "design", &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resume_json_all_phases_valid() {
        // Test that all valid phases can be used in resume JSON output
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let phases = [
            "requirements",
            "design",
            "tasks",
            "review",
            "fixup",
            "final",
        ];

        for phase in &phases {
            let output = ResumeJsonOutput {
                schema_version: "resume-json.v1".to_string(),
                spec_id: "test-spec".to_string(),
                phase: phase.to_string(),
                current_inputs: CurrentInputs {
                    available_artifacts: vec![],
                    spec_exists: true,
                    latest_completed_phase: None,
                },
                next_steps: format!("Run {} phase.", phase),
            };

            let json_result = emit_resume_json(&output);
            assert!(
                json_result.is_ok(),
                "Failed to emit resume JSON for phase: {}",
                phase
            );

            let json_str = json_result.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            assert_eq!(parsed["phase"], *phase);
        }
    }

    #[test]
    fn test_resume_json_spec_not_exists() {
        // Test resume JSON output when spec doesn't exist
        use crate::types::{CurrentInputs, ResumeJsonOutput};

        let output = ResumeJsonOutput {
            schema_version: "resume-json.v1".to_string(),
            spec_id: "nonexistent-spec".to_string(),
            phase: "requirements".to_string(),
            current_inputs: CurrentInputs {
                available_artifacts: vec![],
                spec_exists: false,
                latest_completed_phase: None,
            },
            next_steps: "Spec 'nonexistent-spec' does not exist. Run 'xchecker spec nonexistent-spec' to create it first.".to_string(),
        };

        let json_result = emit_resume_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify spec_exists is false
        assert_eq!(parsed["current_inputs"]["spec_exists"], false);
        // Verify available_artifacts is either empty or not present (due to skip_serializing_if)
        let artifacts = parsed["current_inputs"].get("available_artifacts");
        match artifacts {
            Some(arr) => {
                let arr = arr.as_array().unwrap();
                assert!(arr.is_empty());
            }
            None => {
                // Field is skipped when empty, which is valid
            }
        }
    }

    // ===== Project List Tests (Task 28) =====
    // Tests for derive_spec_status function
    // **Validates: Requirements 4.3.3**

    #[test]
    fn test_derive_spec_status_not_started() {
        // Use isolated home to avoid conflicts with other tests
        let _temp_dir = crate::paths::with_isolated_home();

        // Test status for non-existent spec
        let status = derive_spec_status("nonexistent-spec-status-test");
        assert_eq!(status, "not_started");
    }

    #[test]
    fn test_derive_spec_status_with_receipt() {
        // Use isolated home to avoid conflicts with other tests
        let _temp_dir = crate::paths::with_isolated_home();

        use crate::receipt::ReceiptManager;
        use crate::types::{PacketEvidence, PhaseId};
        use std::collections::HashMap;

        // Create a spec with a receipt
        let spec_id = "test-spec-with-receipt";
        let base_path = crate::paths::spec_root(spec_id);
        crate::paths::ensure_dir_all(&base_path).unwrap();

        let receipt_manager = ReceiptManager::new(&base_path);

        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        // Create a successful receipt
        let receipt = receipt_manager.create_receipt(
            spec_id,
            PhaseId::Requirements,
            0, // exit_code 0 = success
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None,
            None,
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Test status derivation
        let status = derive_spec_status(spec_id);
        assert!(
            status.contains("success"),
            "Expected 'success' in status, got: {}",
            status
        );
        assert!(
            status.contains("requirements"),
            "Expected 'requirements' in status, got: {}",
            status
        );
    }

    #[test]
    fn test_derive_spec_status_with_failed_receipt() {
        // Use isolated home to avoid conflicts with other tests
        let _temp_dir = crate::paths::with_isolated_home();

        use crate::receipt::ReceiptManager;
        use crate::types::{PacketEvidence, PhaseId};
        use std::collections::HashMap;

        // Create a spec with a failed receipt
        let spec_id = "test-spec-with-failed-receipt";
        let base_path = crate::paths::spec_root(spec_id);
        crate::paths::ensure_dir_all(&base_path).unwrap();

        let receipt_manager = ReceiptManager::new(&base_path);

        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        // Create a failed receipt
        let receipt = receipt_manager.create_receipt(
            spec_id,
            PhaseId::Design,
            1, // exit_code 1 = failure
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None,
            None,
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Test status derivation
        let status = derive_spec_status(spec_id);
        assert!(
            status.contains("failed"),
            "Expected 'failed' in status, got: {}",
            status
        );
        assert!(
            status.contains("design"),
            "Expected 'design' in status, got: {}",
            status
        );
    }

    #[test]
    fn test_derive_spec_status_uses_latest_receipt() {
        // Use isolated home to avoid conflicts with other tests
        let _temp_dir = crate::paths::with_isolated_home();

        use crate::receipt::ReceiptManager;
        use crate::types::{PacketEvidence, PhaseId};
        use std::collections::HashMap;

        // Create a spec with multiple receipts
        let spec_id = "test-spec-multiple-receipts";
        let base_path = crate::paths::spec_root(spec_id);
        crate::paths::ensure_dir_all(&base_path).unwrap();

        let receipt_manager = ReceiptManager::new(&base_path);

        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        // Create first receipt (requirements - success)
        let receipt1 = receipt_manager.create_receipt(
            spec_id,
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet.clone(),
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None,
            None,
        );
        receipt_manager.write_receipt(&receipt1).unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Create second receipt (design - success)
        let receipt2 = receipt_manager.create_receipt(
            spec_id,
            PhaseId::Design,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None,
            None,
        );
        receipt_manager.write_receipt(&receipt2).unwrap();

        // Test status derivation - should show design (latest)
        let status = derive_spec_status(spec_id);
        assert!(
            status.contains("design"),
            "Expected 'design' (latest) in status, got: {}",
            status
        );
        assert!(
            status.contains("success"),
            "Expected 'success' in status, got: {}",
            status
        );
    }

    // ===== Workspace Status JSON Output Tests (Task 29) =====
    // **Property: JSON output includes schema version**
    // **Validates: Requirements 4.3.4**

    #[test]
    fn test_workspace_status_json_output_schema_version() {
        // Test that workspace status JSON output includes schema_version field
        use crate::types::{
            WorkspaceSpecStatus, WorkspaceStatusJsonOutput, WorkspaceStatusSummary,
        };

        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: "test-workspace".to_string(),
            workspace_path: "/path/to/workspace.yaml".to_string(),
            specs: vec![WorkspaceSpecStatus {
                spec_id: "spec-1".to_string(),
                tags: vec!["backend".to_string()],
                status: "success".to_string(),
                latest_phase: Some("tasks".to_string()),
                last_activity: Some(chrono::Utc::now()),
                pending_fixups: 0,
                has_errors: false,
            }],
            summary: WorkspaceStatusSummary {
                total_specs: 1,
                successful_specs: 1,
                failed_specs: 0,
                pending_specs: 0,
                not_started_specs: 0,
                stale_specs: 0,
            },
        };

        // Emit as JSON
        let json_result = emit_workspace_status_json(&output);
        assert!(json_result.is_ok(), "Failed to emit workspace status JSON");

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify schema_version is present and correct
        assert_eq!(parsed["schema_version"], "workspace-status-json.v1");
        assert_eq!(parsed["workspace_name"], "test-workspace");
    }

    #[test]
    fn test_workspace_status_json_output_has_required_fields() {
        // Test that workspace status JSON output has all required fields per Requirements 4.3.4
        use crate::types::{
            WorkspaceSpecStatus, WorkspaceStatusJsonOutput, WorkspaceStatusSummary,
        };

        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: "test-workspace".to_string(),
            workspace_path: "/path/to/workspace.yaml".to_string(),
            specs: vec![
                WorkspaceSpecStatus {
                    spec_id: "spec-1".to_string(),
                    tags: vec![],
                    status: "success".to_string(),
                    latest_phase: Some("design".to_string()),
                    last_activity: None,
                    pending_fixups: 0,
                    has_errors: false,
                },
                WorkspaceSpecStatus {
                    spec_id: "spec-2".to_string(),
                    tags: vec!["frontend".to_string()],
                    status: "failed".to_string(),
                    latest_phase: Some("requirements".to_string()),
                    last_activity: None,
                    pending_fixups: 2,
                    has_errors: true,
                },
            ],
            summary: WorkspaceStatusSummary {
                total_specs: 2,
                successful_specs: 1,
                failed_specs: 1,
                pending_specs: 0,
                not_started_specs: 0,
                stale_specs: 0,
            },
        };

        let json_result = emit_workspace_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all required fields are present
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("workspace_name").is_some());
        assert!(parsed.get("workspace_path").is_some());
        assert!(parsed.get("specs").is_some());
        assert!(parsed.get("summary").is_some());

        // Verify summary fields
        let summary = &parsed["summary"];
        assert!(summary.get("total_specs").is_some());
        assert!(summary.get("successful_specs").is_some());
        assert!(summary.get("failed_specs").is_some());
        assert!(summary.get("pending_specs").is_some());
        assert!(summary.get("not_started_specs").is_some());
        assert!(summary.get("stale_specs").is_some());

        // Verify values
        assert_eq!(summary["total_specs"], 2);
        assert_eq!(summary["failed_specs"], 1);
    }

    #[test]
    fn test_workspace_status_json_canonical_format() {
        // Test that workspace status JSON output is in canonical JCS format (no extra whitespace)
        use crate::types::{WorkspaceStatusJsonOutput, WorkspaceStatusSummary};

        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: "test-workspace".to_string(),
            workspace_path: "/path/to/workspace.yaml".to_string(),
            specs: vec![],
            summary: WorkspaceStatusSummary {
                total_specs: 0,
                successful_specs: 0,
                failed_specs: 0,
                pending_specs: 0,
                not_started_specs: 0,
                stale_specs: 0,
            },
        };

        let json_result = emit_workspace_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();

        // Verify canonical JSON properties (no extra whitespace, no newlines)
        assert!(
            !json_str.contains("  "),
            "Canonical JSON should not have indentation"
        );
        assert!(
            !json_str.contains('\n'),
            "Canonical JSON should not have newlines"
        );
    }

    #[test]
    fn test_workspace_status_json_spec_statuses() {
        // Test that spec statuses are correctly represented
        use crate::types::{
            WorkspaceSpecStatus, WorkspaceStatusJsonOutput, WorkspaceStatusSummary,
        };

        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: "test-workspace".to_string(),
            workspace_path: "/path/to/workspace.yaml".to_string(),
            specs: vec![
                WorkspaceSpecStatus {
                    spec_id: "spec-success".to_string(),
                    tags: vec!["tag1".to_string(), "tag2".to_string()],
                    status: "success".to_string(),
                    latest_phase: Some("final".to_string()),
                    last_activity: Some(chrono::Utc::now()),
                    pending_fixups: 0,
                    has_errors: false,
                },
                WorkspaceSpecStatus {
                    spec_id: "spec-failed".to_string(),
                    tags: vec![],
                    status: "failed".to_string(),
                    latest_phase: Some("design".to_string()),
                    last_activity: None,
                    pending_fixups: 3,
                    has_errors: true,
                },
                WorkspaceSpecStatus {
                    spec_id: "spec-not-started".to_string(),
                    tags: vec![],
                    status: "not_started".to_string(),
                    latest_phase: None,
                    last_activity: None,
                    pending_fixups: 0,
                    has_errors: false,
                },
            ],
            summary: WorkspaceStatusSummary {
                total_specs: 3,
                successful_specs: 1,
                failed_specs: 1,
                pending_specs: 0,
                not_started_specs: 1,
                stale_specs: 0,
            },
        };

        let json_result = emit_workspace_status_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify specs array
        let specs = parsed["specs"].as_array().unwrap();
        assert_eq!(specs.len(), 3);

        // Verify spec IDs
        let spec_ids: Vec<&str> = specs
            .iter()
            .map(|s| s["spec_id"].as_str().unwrap())
            .collect();
        assert!(spec_ids.contains(&"spec-success"));
        assert!(spec_ids.contains(&"spec-failed"));
        assert!(spec_ids.contains(&"spec-not-started"));

        // Verify statuses
        let statuses: Vec<&str> = specs
            .iter()
            .map(|s| s["status"].as_str().unwrap())
            .collect();
        assert!(statuses.contains(&"success"));
        assert!(statuses.contains(&"failed"));
        assert!(statuses.contains(&"not_started"));
    }

    #[test]
    fn test_workspace_status_cli_parsing() {
        // Test that CLI arguments are properly parsed for project status command
        use clap::Parser;

        // Test basic project status command
        let args = vec!["xchecker", "project", "status"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Project(ProjectCommands::Status { workspace, json }) => {
                    assert!(workspace.is_none());
                    assert!(!json);
                }
                _ => panic!("Expected Project Status command"),
            }
        }

        // Test project status with --json flag
        let args_json = vec!["xchecker", "project", "status", "--json"];
        let cli_json = Cli::try_parse_from(args_json);
        assert!(cli_json.is_ok());

        if let Ok(cli) = cli_json {
            match cli.command {
                Commands::Project(ProjectCommands::Status { workspace, json }) => {
                    assert!(workspace.is_none());
                    assert!(json);
                }
                _ => panic!("Expected Project Status command"),
            }
        }

        // Test project status with --workspace flag
        let args_workspace = vec![
            "xchecker",
            "project",
            "status",
            "--workspace",
            "/path/to/workspace.yaml",
        ];
        let cli_workspace = Cli::try_parse_from(args_workspace);
        assert!(cli_workspace.is_ok());

        if let Ok(cli) = cli_workspace {
            match cli.command {
                Commands::Project(ProjectCommands::Status { workspace, json }) => {
                    assert!(workspace.is_some());
                    assert_eq!(
                        workspace.unwrap().to_str().unwrap(),
                        "/path/to/workspace.yaml"
                    );
                    assert!(!json);
                }
                _ => panic!("Expected Project Status command"),
            }
        }
    }

    // ===== Project History Tests (Task 30) =====
    // **Validates: Requirements 4.3.5**

    #[test]
    fn test_workspace_history_cli_parsing() {
        // Test that CLI arguments are properly parsed for project history command
        use clap::Parser;

        // Test basic project history command
        let args = vec!["xchecker", "project", "history", "my-spec"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());

        if let Ok(cli) = cli {
            match cli.command {
                Commands::Project(ProjectCommands::History { spec_id, json }) => {
                    assert_eq!(spec_id, "my-spec");
                    assert!(!json);
                }
                _ => panic!("Expected Project History command"),
            }
        }

        // Test project history with --json flag
        let args_json = vec!["xchecker", "project", "history", "my-spec", "--json"];
        let cli_json = Cli::try_parse_from(args_json);
        assert!(cli_json.is_ok());

        if let Ok(cli) = cli_json {
            match cli.command {
                Commands::Project(ProjectCommands::History { spec_id, json }) => {
                    assert_eq!(spec_id, "my-spec");
                    assert!(json);
                }
                _ => panic!("Expected Project History command"),
            }
        }
    }

    #[test]
    fn test_history_json_output_schema_version() {
        // Test that history JSON output includes schema_version field
        use crate::types::{HistoryEntry, HistoryMetrics, WorkspaceHistoryJsonOutput};

        let output = WorkspaceHistoryJsonOutput {
            schema_version: "workspace-history-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            timeline: vec![HistoryEntry {
                phase: "requirements".to_string(),
                timestamp: chrono::Utc::now(),
                exit_code: 0,
                success: true,
                tokens_input: Some(1000),
                tokens_output: Some(500),
                fixup_count: None,
                model: Some("haiku".to_string()),
                provider: Some("claude-cli".to_string()),
            }],
            metrics: HistoryMetrics {
                total_executions: 1,
                successful_executions: 1,
                failed_executions: 0,
                total_tokens_input: 1000,
                total_tokens_output: 500,
                total_fixups: 0,
                first_execution: Some(chrono::Utc::now()),
                last_execution: Some(chrono::Utc::now()),
            },
        };

        // Emit as JSON
        let json_result = emit_workspace_history_json(&output);
        assert!(json_result.is_ok(), "Failed to emit history JSON");

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify schema_version is present and correct
        assert_eq!(parsed["schema_version"], "workspace-history-json.v1");
        assert_eq!(parsed["spec_id"], "test-spec");
    }

    #[test]
    fn test_history_json_output_has_required_fields() {
        // Test that history JSON output has all required fields per Requirements 4.3.5
        use crate::types::{HistoryEntry, HistoryMetrics, WorkspaceHistoryJsonOutput};

        let output = WorkspaceHistoryJsonOutput {
            schema_version: "workspace-history-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            timeline: vec![
                HistoryEntry {
                    phase: "requirements".to_string(),
                    timestamp: chrono::Utc::now(),
                    exit_code: 0,
                    success: true,
                    tokens_input: Some(1000),
                    tokens_output: Some(500),
                    fixup_count: None,
                    model: Some("haiku".to_string()),
                    provider: None,
                },
                HistoryEntry {
                    phase: "design".to_string(),
                    timestamp: chrono::Utc::now(),
                    exit_code: 1,
                    success: false,
                    tokens_input: Some(2000),
                    tokens_output: Some(100),
                    fixup_count: None,
                    model: Some("haiku".to_string()),
                    provider: None,
                },
            ],
            metrics: HistoryMetrics {
                total_executions: 2,
                successful_executions: 1,
                failed_executions: 1,
                total_tokens_input: 3000,
                total_tokens_output: 600,
                total_fixups: 0,
                first_execution: Some(chrono::Utc::now()),
                last_execution: Some(chrono::Utc::now()),
            },
        };

        let json_result = emit_workspace_history_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all required fields are present
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("spec_id").is_some());
        assert!(parsed.get("timeline").is_some());
        assert!(parsed.get("metrics").is_some());

        // Verify metrics structure
        let metrics = &parsed["metrics"];
        assert!(metrics.get("total_executions").is_some());
        assert!(metrics.get("successful_executions").is_some());
        assert!(metrics.get("failed_executions").is_some());
        assert!(metrics.get("total_tokens_input").is_some());
        assert!(metrics.get("total_tokens_output").is_some());
        assert!(metrics.get("total_fixups").is_some());

        // Verify values
        assert_eq!(metrics["total_executions"], 2);
        assert_eq!(metrics["successful_executions"], 1);
        assert_eq!(metrics["failed_executions"], 1);
        assert_eq!(metrics["total_tokens_input"], 3000);
        assert_eq!(metrics["total_tokens_output"], 600);
    }

    #[test]
    fn test_history_json_canonical_format() {
        // Test that history JSON output is in canonical JCS format (no extra whitespace)
        use crate::types::{HistoryMetrics, WorkspaceHistoryJsonOutput};

        let output = WorkspaceHistoryJsonOutput {
            schema_version: "workspace-history-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            timeline: vec![],
            metrics: HistoryMetrics {
                total_executions: 0,
                successful_executions: 0,
                failed_executions: 0,
                total_tokens_input: 0,
                total_tokens_output: 0,
                total_fixups: 0,
                first_execution: None,
                last_execution: None,
            },
        };

        let json_result = emit_workspace_history_json(&output);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();

        // Verify canonical JSON properties (no extra whitespace, no newlines)
        assert!(
            !json_str.contains("  "),
            "Canonical JSON should not have indentation"
        );
        assert!(
            !json_str.contains('\n'),
            "Canonical JSON should not have newlines"
        );
    }

    #[test]
    fn test_history_command_no_spec() {
        let _temp_dir = setup_test_environment();

        // Test history for non-existent spec
        let result = execute_project_history_command("nonexistent-spec-history", false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_history_command_json_no_spec() {
        let _temp_dir = setup_test_environment();

        // Test history --json for non-existent spec
        let result = execute_project_history_command("nonexistent-spec-history-json", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_history_timeline_entry_structure() {
        // Test that timeline entries have correct structure
        use crate::types::HistoryEntry;

        let entry = HistoryEntry {
            phase: "requirements".to_string(),
            timestamp: chrono::Utc::now(),
            exit_code: 0,
            success: true,
            tokens_input: Some(1000),
            tokens_output: Some(500),
            fixup_count: Some(3),
            model: Some("haiku".to_string()),
            provider: Some("openrouter".to_string()),
        };

        // Serialize and verify
        let json_value = serde_json::to_value(&entry).unwrap();

        assert_eq!(json_value["phase"], "requirements");
        assert_eq!(json_value["exit_code"], 0);
        assert_eq!(json_value["success"], true);
        assert_eq!(json_value["tokens_input"], 1000);
        assert_eq!(json_value["tokens_output"], 500);
        assert_eq!(json_value["fixup_count"], 3);
        assert_eq!(json_value["model"], "haiku");
        assert_eq!(json_value["provider"], "openrouter");
    }

    #[test]
    fn test_history_metrics_aggregation() {
        // Test that metrics are correctly aggregated
        use crate::types::HistoryMetrics;

        let metrics = HistoryMetrics {
            total_executions: 5,
            successful_executions: 3,
            failed_executions: 2,
            total_tokens_input: 10000,
            total_tokens_output: 5000,
            total_fixups: 7,
            first_execution: Some(chrono::Utc::now()),
            last_execution: Some(chrono::Utc::now()),
        };

        // Serialize and verify
        let json_value = serde_json::to_value(&metrics).unwrap();

        assert_eq!(json_value["total_executions"], 5);
        assert_eq!(json_value["successful_executions"], 3);
        assert_eq!(json_value["failed_executions"], 2);
        assert_eq!(json_value["total_tokens_input"], 10000);
        assert_eq!(json_value["total_tokens_output"], 5000);
        assert_eq!(json_value["total_fixups"], 7);
    }
}

/// Derive spec status from the latest receipt
///
/// Returns a human-readable status string based on the latest receipt:
/// - "success" if the latest receipt has exit_code 0
/// - "failed" if the latest receipt has non-zero exit_code
/// - "not_started" if no receipts exist
/// - "unknown" if receipts cannot be read
fn derive_spec_status(spec_id: &str) -> String {
    use crate::receipt::ReceiptManager;

    let base_path = crate::paths::spec_root(spec_id);
    let receipt_manager = ReceiptManager::new(&base_path);

    // Try to list all receipts for this spec
    match receipt_manager.list_receipts() {
        Ok(receipts) => {
            if receipts.is_empty() {
                "not_started".to_string()
            } else {
                // Get the latest receipt (list_receipts returns sorted by emitted_at)
                let latest = receipts.last().unwrap();
                if latest.exit_code == 0 {
                    // Include the phase name for more context
                    format!("{}: success", latest.phase)
                } else {
                    format!("{}: failed", latest.phase)
                }
            }
        }
        Err(_) => {
            // Check if the spec directory exists at all
            if base_path.exists() {
                "unknown".to_string()
            } else {
                "not_started".to_string()
            }
        }
    }
}

/// Execute project/workspace management commands
fn execute_project_command(cmd: ProjectCommands) -> Result<()> {
    use crate::workspace::{self, Workspace};

    match cmd {
        ProjectCommands::Init { name } => {
            let cwd = std::env::current_dir().context("Failed to get current directory")?;

            let workspace_path = workspace::init_workspace(&cwd, &name)?;

            println!("✓ Initialized workspace: {}", name);
            println!("  Created: {}", workspace_path.display());
            println!("\nNext steps:");
            println!("  - Add specs with: xchecker project add-spec <spec-id>");
            println!("  - List specs with: xchecker project list");

            Ok(())
        }
        ProjectCommands::AddSpec {
            spec_id,
            tag,
            force,
        } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;

            // Discover workspace
            let workspace_path = workspace::discover_workspace_from_cwd()?.ok_or_else(|| {
                anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
            })?;

            // Load workspace
            let mut ws = Workspace::load(&workspace_path)?;

            // Add spec
            ws.add_spec(&sanitized_id, tag.clone(), force)?;

            // Save workspace
            ws.save(&workspace_path)?;

            println!("✓ Added spec '{}' to workspace", sanitized_id);
            if !tag.is_empty() {
                println!("  Tags: {}", tag.join(", "));
            }

            Ok(())
        }
        ProjectCommands::List { workspace } => {
            // Resolve workspace path
            let workspace_path =
                workspace::resolve_workspace(workspace.as_deref())?.ok_or_else(|| {
                    anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
                })?;

            // Load workspace
            let ws = Workspace::load(&workspace_path)?;

            println!("Workspace: {}", ws.name);
            println!("Location: {}", workspace_path.display());
            println!();

            if ws.specs.is_empty() {
                println!("No specs registered.");
                println!("\nAdd specs with: xchecker project add-spec <spec-id>");
            } else {
                println!("Specs ({}):", ws.specs.len());
                for spec in ws.list_specs() {
                    // Derive status from latest receipt
                    let status = derive_spec_status(&spec.id);

                    let tags_str = if spec.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", spec.tags.join(", "))
                    };

                    // Format: spec-id (status) [tags]
                    println!("  - {} ({}){}", spec.id, status, tags_str);
                }
            }

            Ok(())
        }
        ProjectCommands::Status { workspace, json } => {
            execute_project_status_command(workspace.as_deref(), json)
        }
        ProjectCommands::History { spec_id, json } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;
            execute_project_history_command(&sanitized_id, json)
        }
        ProjectCommands::Tui { workspace } => execute_project_tui_command(workspace.as_deref()),
    }
}

/// Execute the project status command
/// Per FR-WORKSPACE (Requirements 4.3.4): Emits aggregated status for all specs
fn execute_project_status_command(
    workspace_override: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    use crate::receipt::ReceiptManager;
    use crate::types::{WorkspaceSpecStatus, WorkspaceStatusJsonOutput, WorkspaceStatusSummary};
    use crate::workspace::{self, Workspace};

    // Resolve workspace path
    let workspace_path = workspace::resolve_workspace(workspace_override)?.ok_or_else(|| {
        anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
    })?;

    // Load workspace
    let ws = Workspace::load(&workspace_path)?;

    // Collect status for each spec
    let mut spec_statuses = Vec::new();
    let mut summary = WorkspaceStatusSummary {
        total_specs: ws.specs.len() as u32,
        successful_specs: 0,
        failed_specs: 0,
        pending_specs: 0,
        not_started_specs: 0,
        stale_specs: 0,
    };

    // Define stale threshold (7 days)
    let stale_threshold = chrono::Duration::days(7);
    let now = chrono::Utc::now();

    for spec in ws.list_specs() {
        let base_path = crate::paths::spec_root(&spec.id);
        let receipt_manager = ReceiptManager::new(&base_path);

        // Get receipts for this spec
        let receipts = receipt_manager.list_receipts().unwrap_or_default();

        // Determine spec status
        let (status, latest_phase, last_activity, has_errors) = if receipts.is_empty() {
            summary.not_started_specs += 1;
            ("not_started".to_string(), None, None, false)
        } else {
            let latest = receipts.last().unwrap();
            let last_activity_time = latest.emitted_at;
            let is_stale = now.signed_duration_since(last_activity_time) > stale_threshold;

            if is_stale {
                summary.stale_specs += 1;
            }

            if latest.exit_code == 0 {
                // Check if all phases are complete
                let all_phases_complete = receipts
                    .iter()
                    .any(|r| r.phase == "final" && r.exit_code == 0);
                if all_phases_complete {
                    summary.successful_specs += 1;
                    (
                        if is_stale { "stale" } else { "success" }.to_string(),
                        Some(latest.phase.clone()),
                        Some(last_activity_time),
                        false,
                    )
                } else {
                    summary.pending_specs += 1;
                    (
                        if is_stale { "stale" } else { "pending" }.to_string(),
                        Some(latest.phase.clone()),
                        Some(last_activity_time),
                        false,
                    )
                }
            } else {
                summary.failed_specs += 1;
                (
                    "failed".to_string(),
                    Some(latest.phase.clone()),
                    Some(last_activity_time),
                    true,
                )
            }
        };

        // Count pending fixups for this spec
        let pending_fixups = count_pending_fixups_for_spec(&spec.id);

        spec_statuses.push(WorkspaceSpecStatus {
            spec_id: spec.id.clone(),
            tags: spec.tags.clone(),
            status,
            latest_phase,
            last_activity,
            pending_fixups,
            has_errors,
        });
    }

    if json {
        // Emit JSON output
        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: ws.name.clone(),
            workspace_path: workspace_path.display().to_string(),
            specs: spec_statuses,
            summary,
        };

        let json_output = emit_workspace_status_json(&output)?;
        println!("{json_output}");
    } else {
        // Human-readable output
        println!("Workspace: {}", ws.name);
        println!("Location: {}", workspace_path.display());
        println!();

        // Summary
        println!("Summary:");
        println!("  Total specs: {}", summary.total_specs);
        println!("  Successful: {}", summary.successful_specs);
        println!("  Failed: {}", summary.failed_specs);
        println!("  Pending: {}", summary.pending_specs);
        println!("  Not started: {}", summary.not_started_specs);
        if summary.stale_specs > 0 {
            println!("  Stale (>7 days): {}", summary.stale_specs);
        }
        println!();

        if spec_statuses.is_empty() {
            println!("No specs registered.");
            println!("\nAdd specs with: xchecker project add-spec <spec-id>");
        } else {
            println!("Specs:");
            for spec in &spec_statuses {
                let tags_str = if spec.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", spec.tags.join(", "))
                };

                let phase_str = spec.latest_phase.as_deref().unwrap_or("-");
                let fixups_str = if spec.pending_fixups > 0 {
                    format!(" ({} fixups)", spec.pending_fixups)
                } else {
                    String::new()
                };

                // Format: spec-id (status, phase) [tags] (fixups)
                println!(
                    "  - {} ({}, {}){}{}",
                    spec.spec_id, spec.status, phase_str, tags_str, fixups_str
                );
            }
        }
    }

    Ok(())
}

/// Count pending fixups for a spec
fn count_pending_fixups_for_spec(spec_id: &str) -> u32 {
    crate::fixup::pending_fixups_for_spec(spec_id).targets
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785)
fn emit_workspace_status_json(output: &crate::types::WorkspaceStatusJsonOutput) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit workspace status JSON")
}

/// Execute the project history command
/// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
fn execute_project_history_command(spec_id: &str, json: bool) -> Result<()> {
    use crate::receipt::ReceiptManager;
    use crate::types::{HistoryEntry, HistoryMetrics, WorkspaceHistoryJsonOutput};

    // Get spec base path
    let base_path = crate::paths::spec_root(spec_id);

    // Check if spec exists
    if !base_path.exists() {
        if json {
            // Return empty history for non-existent spec
            let output = WorkspaceHistoryJsonOutput {
                schema_version: "workspace-history-json.v1".to_string(),
                spec_id: spec_id.to_string(),
                timeline: vec![],
                metrics: HistoryMetrics {
                    total_executions: 0,
                    successful_executions: 0,
                    failed_executions: 0,
                    total_tokens_input: 0,
                    total_tokens_output: 0,
                    total_fixups: 0,
                    first_execution: None,
                    last_execution: None,
                },
            };
            let json_output = emit_workspace_history_json(&output)?;
            println!("{json_output}");
        } else {
            println!("History for spec: {spec_id}");
            println!("  Status: Spec not found");
            println!("  Directory: {} (does not exist)", base_path);
        }
        return Ok(());
    }

    // Load receipts
    let receipt_manager = ReceiptManager::new(&base_path);
    let receipts = receipt_manager.list_receipts().unwrap_or_default();

    // Build timeline from receipts
    let mut timeline: Vec<HistoryEntry> = Vec::new();
    let mut metrics = HistoryMetrics {
        total_executions: 0,
        successful_executions: 0,
        failed_executions: 0,
        total_tokens_input: 0,
        total_tokens_output: 0,
        total_fixups: 0,
        first_execution: None,
        last_execution: None,
    };

    for receipt in &receipts {
        let success = receipt.exit_code == 0;

        // Extract LLM metadata if available
        let (tokens_input, tokens_output, provider, model) = if let Some(ref llm) = receipt.llm {
            (
                llm.tokens_input,
                llm.tokens_output,
                llm.provider.clone(),
                llm.model_used.clone(),
            )
        } else {
            (None, None, None, Some(receipt.model_full_name.clone()))
        };

        // Count fixups for fixup phase
        let fixup_count = if receipt.phase == "fixup" && success {
            Some(receipt.outputs.len() as u32)
        } else {
            None
        };

        let entry = HistoryEntry {
            phase: receipt.phase.clone(),
            timestamp: receipt.emitted_at,
            exit_code: receipt.exit_code,
            success,
            tokens_input,
            tokens_output,
            fixup_count,
            model,
            provider,
        };

        // Update metrics
        metrics.total_executions += 1;
        if success {
            metrics.successful_executions += 1;
        } else {
            metrics.failed_executions += 1;
        }
        if let Some(ti) = tokens_input {
            metrics.total_tokens_input += ti;
        }
        if let Some(to) = tokens_output {
            metrics.total_tokens_output += to;
        }
        if let Some(fc) = fixup_count {
            metrics.total_fixups += fc;
        }

        // Track first and last execution
        if metrics.first_execution.is_none()
            || receipt.emitted_at < metrics.first_execution.unwrap()
        {
            metrics.first_execution = Some(receipt.emitted_at);
        }
        if metrics.last_execution.is_none() || receipt.emitted_at > metrics.last_execution.unwrap()
        {
            metrics.last_execution = Some(receipt.emitted_at);
        }

        timeline.push(entry);
    }

    // Sort timeline by timestamp (oldest first)
    timeline.sort_by_key(|e| e.timestamp);

    if json {
        let output = WorkspaceHistoryJsonOutput {
            schema_version: "workspace-history-json.v1".to_string(),
            spec_id: spec_id.to_string(),
            timeline,
            metrics,
        };
        let json_output = emit_workspace_history_json(&output)?;
        println!("{json_output}");
    } else {
        // Human-readable output
        println!("History for spec: {spec_id}");
        println!("Location: {}", base_path);
        println!();

        // Summary metrics
        println!("Summary:");
        println!("  Total executions: {}", metrics.total_executions);
        println!("  Successful: {}", metrics.successful_executions);
        println!("  Failed: {}", metrics.failed_executions);
        if metrics.total_tokens_input > 0 || metrics.total_tokens_output > 0 {
            println!(
                "  Total tokens: {} input, {} output",
                metrics.total_tokens_input, metrics.total_tokens_output
            );
        }
        if metrics.total_fixups > 0 {
            println!("  Total fixups applied: {}", metrics.total_fixups);
        }
        if let Some(first) = metrics.first_execution {
            println!(
                "  First execution: {}",
                first.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
        if let Some(last) = metrics.last_execution {
            println!("  Last execution: {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
        }
        println!();

        if timeline.is_empty() {
            println!("No executions recorded.");
        } else {
            println!("Timeline ({} entries):", timeline.len());
            for entry in &timeline {
                let status_icon = if entry.success { "✓" } else { "✗" };
                let tokens_str = match (entry.tokens_input, entry.tokens_output) {
                    (Some(ti), Some(to)) => format!(" [{} in, {} out]", ti, to),
                    (Some(ti), None) => format!(" [{} in]", ti),
                    (None, Some(to)) => format!(" [{} out]", to),
                    (None, None) => String::new(),
                };
                let fixup_str = entry
                    .fixup_count
                    .map(|c| format!(" ({} fixups)", c))
                    .unwrap_or_default();

                println!(
                    "  {} {} {} (exit {}){}{}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    status_icon,
                    entry.phase,
                    entry.exit_code,
                    tokens_str,
                    fixup_str
                );
            }
        }
    }

    Ok(())
}

/// Emit workspace history output as canonical JSON using JCS (RFC 8785)
fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit workspace history JSON")
}

/// Execute the project TUI command
/// Per FR-WORKSPACE-TUI (Requirements 4.4.1, 4.4.2, 4.4.3): Interactive terminal UI
fn execute_project_tui_command(workspace_override: Option<&std::path::Path>) -> Result<()> {
    use crate::workspace;

    // Resolve workspace path
    let workspace_path = workspace::resolve_workspace(workspace_override)?.ok_or_else(|| {
        anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
    })?;

    // Run the TUI
    crate::tui::run_tui(&workspace_path)
}

/// Execute template management commands
/// Per FR-TEMPLATES (Requirements 4.7.1, 4.7.2, 4.7.3)
fn execute_template_command(cmd: TemplateCommands) -> Result<()> {
    use crate::template;

    match cmd {
        TemplateCommands::List => {
            println!("Available templates:\n");

            for t in template::list_templates() {
                println!("  {}", t.id);
                println!("    Name: {}", t.name);
                println!("    Description: {}", t.description);
                println!("    Use case: {}", t.use_case);
                if !t.prerequisites.is_empty() {
                    println!("    Prerequisites: {}", t.prerequisites.join(", "));
                }
                println!();
            }

            println!("To initialize a spec from a template:");
            println!("  xchecker template init <template> <spec-id>");

            Ok(())
        }
        TemplateCommands::Init { template, spec_id } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;

            // Validate template
            if !template::is_valid_template(&template) {
                let valid_templates = template::BUILT_IN_TEMPLATES.join(", ");
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "template".to_string(),
                    value: format!(
                        "Unknown template '{}'. Valid templates: {}",
                        template, valid_templates
                    ),
                })
                .into());
            }

            // Initialize from template
            template::init_from_template(&template, &sanitized_id)?;

            // Get template info for display
            let template_info = template::get_template(&template).unwrap();

            println!(
                "✓ Initialized spec '{}' from template '{}'",
                sanitized_id, template
            );
            println!();
            println!("Template: {}", template_info.name);
            println!("Description: {}", template_info.description);
            println!();
            println!("Created files:");
            println!(
                "  - .xchecker/specs/{}/context/problem-statement.md",
                sanitized_id
            );
            println!("  - .xchecker/specs/{}/README.md", sanitized_id);
            println!();
            println!("Next steps:");
            println!("  1. Review the problem statement:");
            println!(
                "     cat .xchecker/specs/{}/context/problem-statement.md",
                sanitized_id
            );
            println!("  2. Customize the problem statement for your needs");
            println!("  3. Run the requirements phase:");
            println!("     xchecker resume {} --phase requirements", sanitized_id);

            Ok(())
        }
    }
}
