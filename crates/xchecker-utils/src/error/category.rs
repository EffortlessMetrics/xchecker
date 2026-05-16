use std::fmt;

/// Categories of errors for better organization and handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    Configuration,
    PhaseExecution,
    ClaudeIntegration,
    FileSystem,
    Security,
    ResourceLimits,
    Concurrency,
    Validation,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration => write!(f, "Configuration"),
            Self::PhaseExecution => write!(f, "Phase Execution"),
            Self::ClaudeIntegration => write!(f, "Claude Integration"),
            Self::FileSystem => write!(f, "File System"),
            Self::Security => write!(f, "Security"),
            Self::ResourceLimits => write!(f, "Resource Limits"),
            Self::Concurrency => write!(f, "Concurrency"),
            Self::Validation => write!(f, "Validation"),
        }
    }
}
