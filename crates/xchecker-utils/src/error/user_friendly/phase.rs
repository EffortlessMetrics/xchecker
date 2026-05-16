use super::UserFriendlyError;
use crate::error::{ErrorCategory, PhaseError};

impl UserFriendlyError for PhaseError {
    fn user_message(&self) -> String {
        match self {
            Self::ExecutionFailed { phase, code } => {
                format!("The {phase} phase failed to complete successfully (exit code: {code})")
            }
            Self::DependencyNotSatisfied { phase, dependency } => {
                format!(
                    "Cannot run {phase} phase: {dependency} phase must complete successfully first"
                )
            }
            Self::InvalidTransition { from, to } => {
                format!("Cannot transition from {from} phase to {to} phase")
            }
            Self::OutputValidationFailed { phase, reason } => {
                format!("The {phase} phase produced invalid output: {reason}")
            }
            Self::PacketCreationFailed { phase, reason } => {
                format!("Failed to create input packet for {phase} phase: {reason}")
            }
            Self::ContextCreationFailed { phase, reason } => {
                format!("Failed to create execution context for {phase} phase: {reason}")
            }
            Self::Timeout {
                phase,
                timeout_seconds,
            } => {
                format!("The {phase} phase timed out after {timeout_seconds} seconds")
            }
            Self::Interrupted { phase } => {
                format!("The {phase} phase was interrupted")
            }
            Self::ResourceLimitExceeded {
                phase,
                resource,
                limit,
            } => {
                format!("The {phase} phase exceeded {resource} limit: {limit}")
            }
            Self::ExecutionFailedWithStderr {
                phase,
                code,
                stderr_tail,
            } => {
                format!(
                    "The {phase} phase failed (exit code: {code}) with error output: {stderr_tail}"
                )
            }
            Self::PartialOutputSaved {
                phase,
                partial_path,
            } => {
                format!("The {phase} phase failed and partial output was saved to: {partial_path}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::ExecutionFailed { phase, code: _ } => {
                Some(format!("The {phase} phase encountered an error during execution. Partial outputs have been saved for debugging."))
            }
            Self::DependencyNotSatisfied { phase: _, dependency: _ } => {
                Some("xchecker phases have dependencies that must be satisfied before execution.".to_string())
            }
            Self::InvalidTransition { from: _, to: _ } => {
                Some("Phase transitions must follow the defined workflow order.".to_string())
            }
            Self::OutputValidationFailed { phase: _, reason: _ } => {
                Some("Phase outputs are validated to ensure they meet the expected format and structure.".to_string())
            }
            Self::PacketCreationFailed { phase: _, reason: _ } => {
                Some("Input packets contain the context and files needed for Claude to generate phase outputs.".to_string())
            }
            Self::ContextCreationFailed { phase: _, reason: _ } => {
                Some("Execution context includes configuration, artifacts, and environment needed for phase execution.".to_string())
            }
            Self::Timeout { phase: _, timeout_seconds: _ } => {
                Some("Phase execution has configurable timeouts to prevent hanging operations.".to_string())
            }
            Self::Interrupted { phase: _ } => {
                Some("Phase execution can be interrupted by user signals (Ctrl+C) or system events.".to_string())
            }
            Self::ResourceLimitExceeded { phase: _, resource: _, limit: _ } => {
                Some("Resource limits prevent excessive memory, disk, or network usage during phase execution.".to_string())
            }
            Self::ExecutionFailedWithStderr { phase: _, code: _, stderr_tail: _ } => {
                Some("Phase execution failed and stderr output has been captured for debugging.".to_string())
            }
            Self::PartialOutputSaved { phase: _, partial_path: _ } => {
                Some("Partial outputs are saved when phases fail to help with debugging and recovery.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::ExecutionFailed { phase, code: _ } => {
                let mut suggestions = vec![
                    format!(
                        "Check the partial output in .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                        phase.to_lowercase()
                    ),
                    "Review the receipt file for detailed error information".to_string(),
                    "Check the stderr output in the receipt for Claude CLI errors".to_string(),
                ];

                match phase.as_str() {
                    "REQUIREMENTS" => {
                        suggestions.push(
                            "Ensure your problem statement is clear and well-defined".to_string(),
                        );
                        suggestions.push("Try simplifying the requirements scope".to_string());
                    }
                    "DESIGN" => {
                        suggestions.push("Review the requirements for completeness".to_string());
                        suggestions
                            .push("Check if the design complexity is appropriate".to_string());
                    }
                    "TASKS" => {
                        suggestions.push("Verify the design document is complete".to_string());
                        suggestions.push("Consider breaking down complex tasks".to_string());
                    }
                    _ => {}
                }

                suggestions
            }
            Self::DependencyNotSatisfied {
                phase: _,
                dependency,
            } => vec![
                format!("Run the {} phase first: xchecker spec <id>", dependency),
                format!("Check the status of {}: xchecker status <id>", dependency),
                "Ensure the dependency phase completed successfully (exit code 0)".to_string(),
            ],
            Self::InvalidTransition { from: _, to: _ } => vec![
                "Follow the standard phase order: Requirements → Design → Tasks → Review → Fixup"
                    .to_string(),
                "Use 'xchecker resume <id> --phase <name>' to restart from a specific phase"
                    .to_string(),
                "Check 'xchecker status <id>' to see the current phase state".to_string(),
            ],
            Self::OutputValidationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check the phase output format matches the expected structure".to_string(),
                "Verify that required sections are present in the output".to_string(),
                "Review the canonicalization requirements for the output type".to_string(),
            ],
            Self::PacketCreationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check that all required input files are accessible".to_string(),
                "Verify packet size limits are not exceeded".to_string(),
                "Ensure no secrets are detected in the input files".to_string(),
                "Review include/exclude patterns in configuration".to_string(),
            ],
            Self::ContextCreationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check that the spec directory is accessible and writable".to_string(),
                "Verify configuration values are valid".to_string(),
                "Ensure previous phase artifacts are available if required".to_string(),
                "Check file permissions in the working directory".to_string(),
            ],
            Self::Timeout {
                phase,
                timeout_seconds: _,
            } => vec![
                format!("Increase timeout for {} phase in configuration", phase),
                "Check your internet connection if using Claude API".to_string(),
                "Try running with --verbose to see where it's hanging".to_string(),
                "Consider breaking down complex requests into smaller parts".to_string(),
            ],
            Self::Interrupted { phase: _ } => vec![
                "Resume the phase with: xchecker resume <id> --phase <name>".to_string(),
                "Check partial outputs in .xchecker/specs/<id>/artifacts/".to_string(),
                "Use --dry-run to test without making Claude calls".to_string(),
            ],
            Self::ResourceLimitExceeded {
                phase: _,
                resource,
                limit: _,
            } => match resource.as_str() {
                "memory" => vec![
                    "Reduce packet size limits in configuration".to_string(),
                    "Use more restrictive file include patterns".to_string(),
                    "Process files in smaller batches".to_string(),
                ],
                "disk" => vec![
                    "Free up disk space".to_string(),
                    "Clean old spec artifacts with: xchecker clean <id>".to_string(),
                    "Check available disk space".to_string(),
                ],
                "network" => vec![
                    "Check your internet connection".to_string(),
                    "Verify Claude API rate limits".to_string(),
                    "Try again later if rate limited".to_string(),
                ],
                _ => vec![
                    "Check system resources and limits".to_string(),
                    "Review configuration for resource settings".to_string(),
                ],
            },
            Self::ExecutionFailedWithStderr {
                phase,
                code: _,
                stderr_tail: _,
            } => vec![
                format!(
                    "Check the stderr output captured in the receipt for detailed error information"
                ),
                format!(
                    "Review partial outputs in .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                    phase.to_lowercase()
                ),
                "Try running with --dry-run to test configuration without Claude calls".to_string(),
                "Check Claude CLI authentication and connectivity".to_string(),
            ],
            Self::PartialOutputSaved {
                phase: _,
                partial_path,
            } => vec![
                format!("Review the partial output at: {}", partial_path),
                "Check the receipt file for detailed error information and stderr output"
                    .to_string(),
                "Use the partial output to understand where the phase failed".to_string(),
                "Try resuming the phase after addressing any issues".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::PhaseExecution
    }
}
