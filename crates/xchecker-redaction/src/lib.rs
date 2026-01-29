//! Secret redaction system for protecting sensitive information in packets
//!
//! This module implements configurable secret pattern detection and redaction
//! to prevent sensitive information from being included in Claude CLI packets.

use anyhow::{Context, Result};
use regex::{Regex, RegexSet};
use std::collections::HashMap;
use std::sync::LazyLock;

// =========================================================================
// Canonical Pattern Definitions
// =========================================================================

/// Definition of a secret pattern for documentation and runtime use.
///
/// This struct provides the canonical, single source of truth for all secret
/// pattern definitions. The same definitions are used for:
/// - Runtime secret detection and redaction
/// - Documentation generation (docs/SECURITY.md)
/// - Test validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretPatternDef {
    /// Unique identifier for the pattern (e.g., "aws_access_key")
    pub id: &'static str,
    /// Category for grouping in documentation (e.g., "AWS Credentials")
    pub category: &'static str,
    /// The regex pattern string
    pub regex: &'static str,
    /// Human-readable description for documentation
    pub description: &'static str,
}

/// Configuration provider for secret redaction settings.
///
/// This trait keeps `SecretRedactor` decoupled from the concrete config type
/// while allowing `Config` to opt in via an impl in the config crate.
pub trait SecretConfigProvider {
    fn extra_secret_patterns(&self) -> &[String];
    fn ignore_secret_patterns(&self) -> &[String];
}

/// Canonical list of all default secret patterns.
///
/// This is the authoritative source for all built-in secret patterns.
/// Any changes here automatically propagate to:
/// - `SecretRedactor::new()` (runtime detection)
/// - Documentation via `regenerate_secret_patterns_docs`
/// - Test validation via doc_validation tests
pub static DEFAULT_SECRET_PATTERNS: &[SecretPatternDef] = &[
    // =========================================================================
    // AWS Credentials (5 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "aws_access_key",
        category: "AWS Credentials",
        regex: r"AKIA[0-9A-Z]{16}",
        description: "AWS access key IDs",
    },
    SecretPatternDef {
        id: "aws_secret_key",
        category: "AWS Credentials",
        regex: r"AWS_SECRET_ACCESS_KEY[=:][A-Za-z0-9/+=]{40}",
        description: "Secret access key assignments",
    },
    SecretPatternDef {
        id: "aws_secret_key_value",
        category: "AWS Credentials",
        regex: r"(?i)(?:aws_secret|secret_access_key)[=:][A-Za-z0-9/+=]{40}",
        description: "Standalone secret key values",
    },
    SecretPatternDef {
        id: "aws_session_token",
        category: "AWS Credentials",
        regex: r"(?i)AWS_SESSION_TOKEN[=:][A-Za-z0-9/+=]{100,}",
        description: "Session token assignments",
    },
    SecretPatternDef {
        id: "aws_session_token_value",
        category: "AWS Credentials",
        regex: r"(?i)(?:session_token|security_token)[=:][A-Za-z0-9/+=]{100,}",
        description: "Session token values",
    },
    // =========================================================================
    // GCP Credentials (3 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "gcp_service_account_key",
        category: "GCP Credentials",
        regex: r"-----BEGIN (RSA )?PRIVATE KEY-----",
        description: "Service account private key markers",
    },
    SecretPatternDef {
        id: "gcp_api_key",
        category: "GCP Credentials",
        regex: r"AIza[0-9A-Za-z_-]{35}",
        description: "Google API keys",
    },
    SecretPatternDef {
        id: "gcp_oauth_client_secret",
        category: "GCP Credentials",
        regex: r"(?i)client_secret[=:][A-Za-z0-9_-]{24,}",
        description: "OAuth client secrets",
    },
    // =========================================================================
    // Azure Credentials (4 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "azure_storage_key",
        category: "Azure Credentials",
        regex: r"(?i)(?:AccountKey|storage_key)[=:][A-Za-z0-9/+=]{86,90}",
        description: "Storage account keys",
    },
    SecretPatternDef {
        id: "azure_connection_string",
        category: "Azure Credentials",
        regex: r"DefaultEndpointsProtocol=https?;AccountName=[^;]+;AccountKey=[A-Za-z0-9/+=]{86,90}",
        description: "Full connection strings",
    },
    SecretPatternDef {
        id: "azure_sas_token",
        category: "Azure Credentials",
        regex: r"[?&]sig=[A-Za-z0-9%/+=]{40,}",
        description: "Shared Access Signature tokens",
    },
    SecretPatternDef {
        id: "azure_client_secret",
        category: "Azure Credentials",
        regex: r"(?i)(?:AZURE_CLIENT_SECRET|client_secret)[=:][A-Za-z0-9~._-]{34,}",
        description: "Client secrets",
    },
    // =========================================================================
    // Generic API Tokens (5 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "bearer_token",
        category: "Generic API Tokens",
        regex: r"Bearer [A-Za-z0-9._-]{20,}",
        description: "Bearer authentication tokens",
    },
    SecretPatternDef {
        id: "api_key_header",
        category: "Generic API Tokens",
        regex: r"(?i)(?:x-api-key|api-key|apikey)[=:][A-Za-z0-9_-]{20,}",
        description: "API key headers",
    },
    SecretPatternDef {
        id: "authorization_basic",
        category: "Generic API Tokens",
        regex: r"Basic [A-Za-z0-9+/=]{20,}",
        description: "Basic auth credentials",
    },
    SecretPatternDef {
        id: "oauth_token",
        category: "Generic API Tokens",
        regex: r"(?i)(?:access_token|refresh_token)[=:][A-Za-z0-9._-]{20,}",
        description: "OAuth tokens",
    },
    SecretPatternDef {
        id: "jwt_token",
        category: "Generic API Tokens",
        regex: r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*",
        description: "JSON Web Tokens",
    },
    // =========================================================================
    // LLM Provider Tokens (4 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "anthropic_api_key",
        category: "LLM Provider Tokens",
        regex: r"sk-ant-api03-[A-Za-z0-9_-]{20,}",
        description: "Anthropic API keys",
    },
    SecretPatternDef {
        id: "openai_api_key",
        category: "LLM Provider Tokens",
        regex: r"sk-(?:proj|org)-[A-Za-z0-9_-]{20,}",
        description: "OpenAI Project/Org API keys",
    },
    SecretPatternDef {
        id: "openai_legacy_key",
        category: "LLM Provider Tokens",
        regex: r"sk-[A-Za-z0-9]{48}",
        description: "OpenAI Legacy API keys",
    },
    SecretPatternDef {
        id: "huggingface_token",
        category: "LLM Provider Tokens",
        regex: r"hf_[A-Za-z0-9]{34}",
        description: "Hugging Face access tokens",
    },
    // =========================================================================
    // Database Connection URLs (5 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "postgres_url",
        category: "Database Connection URLs",
        regex: r"postgres(?:ql)?://[^:]+:[^@]+@[^\s]+",
        description: "PostgreSQL URLs with credentials",
    },
    SecretPatternDef {
        id: "mysql_url",
        category: "Database Connection URLs",
        regex: r"mysql://[^:]+:[^@]+@[^\s]+",
        description: "MySQL URLs with credentials",
    },
    SecretPatternDef {
        id: "sqlserver_url",
        category: "Database Connection URLs",
        regex: r"(?:sqlserver|mssql)://[^:]+:[^@]+@[^\s]+",
        description: "SQL Server URLs with credentials",
    },
    SecretPatternDef {
        id: "mongodb_url",
        category: "Database Connection URLs",
        regex: r"mongodb(\+srv)?://[^:]+:[^@]+@[^\s]+",
        description: "MongoDB URLs with credentials",
    },
    SecretPatternDef {
        id: "redis_url",
        category: "Database Connection URLs",
        regex: r"rediss?://[^:]*:[^@]+@[^\s]+",
        description: "Redis URLs with credentials",
    },
    // =========================================================================
    // SSH and PEM Private Keys (6 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "age_secret_key",
        category: "SSH and PEM Private Keys",
        regex: r"AGE-SECRET-KEY-1[a-z0-9]{58}",
        description: "Age encryption secret keys",
    },
    SecretPatternDef {
        id: "ssh_private_key",
        category: "SSH and PEM Private Keys",
        regex: r"-----BEGIN (?:OPENSSH |DSA |EC |RSA )?PRIVATE KEY-----",
        description: "SSH private key markers",
    },
    SecretPatternDef {
        id: "rsa_private_key",
        category: "SSH and PEM Private Keys",
        regex: r"-----BEGIN RSA PRIVATE KEY-----",
        description: "RSA private key markers",
    },
    SecretPatternDef {
        id: "ec_private_key",
        category: "SSH and PEM Private Keys",
        regex: r"-----BEGIN EC PRIVATE KEY-----",
        description: "EC private key markers",
    },
    SecretPatternDef {
        id: "pem_private_key",
        category: "SSH and PEM Private Keys",
        regex: r"-----BEGIN PRIVATE KEY-----",
        description: "Generic PEM private key markers",
    },
    SecretPatternDef {
        id: "openssh_private_key",
        category: "SSH and PEM Private Keys",
        regex: r"-----BEGIN OPENSSH PRIVATE KEY-----",
        description: "OpenSSH format markers",
    },
    // =========================================================================
    // Platform-Specific Tokens (13 patterns)
    // =========================================================================
    SecretPatternDef {
        id: "hashicorp_vault_token",
        category: "Platform-Specific Tokens",
        regex: r"hv[bs]\.[a-zA-Z0-9_-]{20,}",
        description: "HashiCorp Vault tokens",
    },
    SecretPatternDef {
        id: "github_pat",
        category: "Platform-Specific Tokens",
        regex: r"ghp_[A-Za-z0-9]{36}",
        description: "GitHub personal access tokens",
    },
    SecretPatternDef {
        id: "github_oauth",
        category: "Platform-Specific Tokens",
        regex: r"gho_[A-Za-z0-9]{36}",
        description: "GitHub OAuth tokens",
    },
    SecretPatternDef {
        id: "github_app_token",
        category: "Platform-Specific Tokens",
        regex: r"gh[us]_[A-Za-z0-9]{36}",
        description: "GitHub App tokens",
    },
    SecretPatternDef {
        id: "gitlab_token",
        category: "Platform-Specific Tokens",
        regex: r"glpat-[A-Za-z0-9_-]{20,}",
        description: "GitLab personal/project tokens",
    },
    SecretPatternDef {
        id: "slack_token",
        category: "Platform-Specific Tokens",
        regex: r"xox[baprs]-[A-Za-z0-9-]+",
        description: "Slack bot/user tokens",
    },
    SecretPatternDef {
        id: "stripe_key",
        category: "Platform-Specific Tokens",
        regex: r"sk_(?:live|test)_[A-Za-z0-9]{24,}",
        description: "Stripe API keys",
    },
    SecretPatternDef {
        id: "twilio_key",
        category: "Platform-Specific Tokens",
        regex: r"SK[A-Za-z0-9]{32}",
        description: "Twilio API keys",
    },
    SecretPatternDef {
        id: "sendgrid_key",
        category: "Platform-Specific Tokens",
        regex: r"SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}",
        description: "SendGrid API keys",
    },
    SecretPatternDef {
        id: "npm_token",
        category: "Platform-Specific Tokens",
        regex: r"npm_[A-Za-z0-9]{36}",
        description: "NPM authentication tokens",
    },
    SecretPatternDef {
        id: "pypi_token",
        category: "Platform-Specific Tokens",
        regex: r"pypi-[A-Za-z0-9_-]{50,}",
        description: "PyPI API tokens",
    },
    SecretPatternDef {
        id: "nuget_key",
        category: "Platform-Specific Tokens",
        regex: r"(?i)nuget_?(?:api_?)?key[=:][A-Za-z0-9]{46}",
        description: "NuGet API keys",
    },
    SecretPatternDef {
        id: "docker_auth",
        category: "Platform-Specific Tokens",
        regex: r#""auth":\s*"[A-Za-z0-9+/=]{20,}""#,
        description: "Docker registry auth tokens",
    },
];

/// Returns the canonical list of default secret pattern definitions.
///
/// This function provides access to the static pattern definitions for use in:
/// - Documentation generation (regenerate_secret_patterns_docs binary)
/// - Test validation (doc_validation tests)
/// - Runtime introspection
///
/// # Example
/// ```
/// use xchecker_redaction::default_pattern_defs;
///
/// let defs = default_pattern_defs();
/// assert!(!defs.is_empty());
///
/// // Patterns are grouped by category
/// let aws_patterns: Vec<_> = defs.iter()
///     .filter(|p| p.category == "AWS Credentials")
///     .collect();
/// assert_eq!(aws_patterns.len(), 5);
/// ```
#[must_use]
pub fn default_pattern_defs() -> &'static [SecretPatternDef] {
    DEFAULT_SECRET_PATTERNS
}

/// Secret redactor with configurable patterns for detecting and redacting sensitive information
#[derive(Debug, Clone)]
pub struct SecretRedactor {
    /// Default secret patterns with their IDs
    default_patterns: HashMap<String, Regex>,
    /// Extra patterns added via configuration
    extra_patterns: HashMap<String, Regex>,
    /// Patterns to ignore (suppress detection)
    ignored_patterns: Vec<String>,

    // Optimization: RegexSet for fast pre-filtering
    // and a parallel list of (ID, Regex) corresponding to the set indices
    regex_set: RegexSet,
    patterns_linear: Vec<(String, Regex)>,
}

/// Information about a detected secret
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMatch {
    /// Pattern ID that matched
    pub pattern_id: String,
    /// File path where secret was found
    pub file_path: String,
    /// Line number (1-based)
    pub line_number: usize,
    /// Column range within the line
    pub column_range: (usize, usize),
    /// Context around the match (never includes the actual secret)
    pub context: String,
}

/// Result of redaction operation
#[derive(Debug, Clone)]
pub struct RedactionResult {
    /// Redacted content with secrets replaced
    pub content: String,
    /// List of detected secrets (for logging)
    #[allow(dead_code)] // Reserved for detailed redaction reporting
    pub matches: Vec<SecretMatch>,
    /// Whether any secrets were found and redacted
    #[allow(dead_code)] // Reserved for structured reporting
    pub has_secrets: bool,
}

impl SecretRedactor {
    /// Create a new `SecretRedactor` with default patterns.
    ///
    /// This constructor compiles all patterns from [`DEFAULT_SECRET_PATTERNS`],
    /// the canonical source of truth for secret pattern definitions.
    ///
    /// See [`default_pattern_defs()`] to access the pattern definitions
    /// for documentation generation or introspection.
    ///
    /// # Errors
    ///
    /// Returns an error if any pattern regex fails to compile (should never
    /// happen with the built-in patterns, but regex compilation is fallible).
    pub fn new() -> Result<Self> {
        let mut default_patterns = HashMap::new();

        for def in DEFAULT_SECRET_PATTERNS {
            let regex = Regex::new(def.regex)
                .with_context(|| format!("Failed to compile {} regex: {}", def.id, def.regex))?;
            default_patterns.insert(def.id.to_string(), regex);
        }

        let mut redactor = Self {
            default_patterns,
            extra_patterns: HashMap::new(),
            ignored_patterns: Vec::new(),
            regex_set: RegexSet::empty(),
            patterns_linear: Vec::new(),
        };

        redactor.rebuild_regex_set()?;

        Ok(redactor)
    }

    /// Rebuilds the internal RegexSet and linear pattern list.
    /// This should be called whenever patterns are added or ignored.
    fn rebuild_regex_set(&mut self) -> Result<()> {
        let mut patterns_to_compile = Vec::new();
        let mut linear = Vec::new();

        // Collect all patterns (default + extra)
        let mut all_patterns: Vec<(&String, &Regex)> = self
            .default_patterns
            .iter()
            .chain(self.extra_patterns.iter())
            .collect();

        // Sort by ID for deterministic behavior
        all_patterns.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

        for (id, regex) in all_patterns {
            if self.is_pattern_ignored(id) {
                continue;
            }
            patterns_to_compile.push(regex.as_str());
            linear.push((id.clone(), regex.clone()));
        }

        self.regex_set = RegexSet::new(patterns_to_compile)
            .context("Failed to compile RegexSet for secret redaction")?;
        self.patterns_linear = linear;

        Ok(())
    }

    /// Create a `SecretRedactor` from a `Config`.
    ///
    /// This method creates a redactor with the default patterns plus any
    /// extra patterns and ignore patterns specified in the config's security section.
    ///
    /// # Arguments
    /// * `config` - The configuration containing security settings
    ///
    /// # Returns
    /// A configured `SecretRedactor` instance
    ///
    /// # Errors
    /// Returns an error if any of the extra patterns fail to compile as regex.
    ///
    /// # Example
    /// ```rust
    /// use xchecker_redaction::{SecretConfigProvider, SecretRedactor};
    ///
    /// struct RedactionConfig {
    ///     extra: Vec<String>,
    ///     ignore: Vec<String>,
    /// }
    ///
    /// impl SecretConfigProvider for RedactionConfig {
    ///     fn extra_secret_patterns(&self) -> &[String] {
    ///         &self.extra
    ///     }
    ///
    ///     fn ignore_secret_patterns(&self) -> &[String] {
    ///         &self.ignore
    ///     }
    /// }
    ///
    /// let config = RedactionConfig {
    ///     extra: vec!["CUSTOM_[A-Z0-9]{32}".to_string()],
    ///     ignore: vec!["test_token".to_string()],
    /// };
    ///
    /// let redactor = SecretRedactor::from_config(&config)
    ///     .expect("Failed to create redactor");
    /// ```
    pub fn from_config<T: SecretConfigProvider>(config: &T) -> Result<Self> {
        let mut redactor = Self::new()?;

        // Add ignored patterns from config first (so they are excluded from rebuilds)
        for pattern_id in config.ignore_secret_patterns() {
            // We use internal method to avoid rebuilding on every add
            redactor.ignored_patterns.push(pattern_id.to_string());
        }

        // Add extra patterns from config
        // This will trigger rebuilds, but usually there are few extra patterns
        for (idx, pattern) in config.extra_secret_patterns().iter().enumerate() {
            let pattern_id = format!("extra_pattern_{}", idx);
            redactor.add_extra_pattern(pattern_id, pattern)?;
        }

        // Rebuild once at the end if we only added ignored patterns and no extra patterns
        // (add_extra_pattern calls rebuild, but if loops are empty or only ignored patterns added...)
        // Actually, let's just make sure we rebuild.
        redactor.rebuild_regex_set()?;

        Ok(redactor)
    }

    /// Redact secrets from a string, replacing them with *** (simplified version for user-facing strings)
    ///
    /// This is a lightweight redaction function for use in error messages, logs, and other
    /// user-facing output. It replaces detected secrets with "***" without detailed tracking.
    ///
    /// # Arguments
    /// * `text` - The text to redact
    ///
    /// # Returns
    /// The redacted text with secrets replaced by "***"
    #[must_use]
    pub fn redact_string(&self, text: &str) -> String {
        let matches = self.regex_set.matches(text);
        if !matches.matched_any() {
            return text.to_string();
        }

        let mut redacted = text.to_string();

        for index in matches.iter() {
            if let Some((_, regex)) = self.patterns_linear.get(index) {
                redacted = regex.replace_all(&redacted, "***").to_string();
            }
        }

        redacted
    }

    /// Redact secrets from a vector of strings
    /// Extended API for batch operations
    ///
    /// # Arguments
    /// * `strings` - Vector of strings to redact
    ///
    /// # Returns
    /// Vector of redacted strings
    #[must_use]
    #[allow(dead_code)] // Extended API for batch redaction
    pub fn redact_strings(&self, strings: &[String]) -> Vec<String> {
        strings.iter().map(|s| self.redact_string(s)).collect()
    }

    /// Redact secrets from an optional string
    /// Extended API for optional field handling
    ///
    /// # Arguments
    /// * `text` - Optional string to redact
    ///
    /// # Returns
    /// Optional redacted string (None if input was None)
    #[must_use]
    #[allow(dead_code)] // Extended API for optional fields
    pub fn redact_optional(&self, text: &Option<String>) -> Option<String> {
        text.as_ref().map(|s| self.redact_string(s))
    }

    /// Add an extra secret pattern to detect
    /// Extended API for custom patterns
    #[allow(dead_code)] // Extended API for custom pattern configuration
    pub fn add_extra_pattern(&mut self, pattern_id: String, pattern: &str) -> Result<()> {
        let regex = Regex::new(pattern).with_context(|| {
            format!("Failed to compile extra pattern '{pattern_id}': {pattern}")
        })?;

        self.extra_patterns.insert(pattern_id, regex);
        self.rebuild_regex_set()?;
        Ok(())
    }

    /// Add a pattern to ignore (suppress detection)
    /// Extended API for pattern suppression
    #[allow(dead_code)] // Extended API for pattern configuration
    pub fn add_ignored_pattern(&mut self, pattern: String) {
        self.ignored_patterns.push(pattern);
        // We must rebuild because ignored patterns are excluded from the set
        let _ = self.rebuild_regex_set();
    }

    /// Scan content for secrets and return matches without redacting
    pub fn scan_for_secrets(&self, content: &str, file_path: &str) -> Result<Vec<SecretMatch>> {
        // Optimization: Use RegexSet to check which patterns match before iterating
        let matches = self.regex_set.matches(content);
        if !matches.matched_any() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for index in matches.iter() {
            if let Some((pattern_id, regex)) = self.patterns_linear.get(index) {
                let pattern_matches =
                    self.find_matches_in_content(content, file_path, pattern_id, regex)?;
                results.extend(pattern_matches);
            }
        }

        Ok(results)
    }

    /// Redact secrets from content, replacing them with placeholder text
    pub fn redact_content(&self, content: &str, file_path: &str) -> Result<RedactionResult> {
        let matches = self.scan_for_secrets(content, file_path)?;

        if matches.is_empty() {
            return Ok(RedactionResult {
                content: content.to_string(),
                matches,
                has_secrets: false,
            });
        }

        // Sort matches by position (reverse order to maintain indices during replacement)
        let mut sorted_matches = matches.clone();
        sorted_matches.sort_by(|a, b| {
            b.line_number
                .cmp(&a.line_number)
                .then_with(|| b.column_range.0.cmp(&a.column_range.0))
        });

        let mut redacted_content = content.to_string();
        let lines: Vec<&str> = content.lines().collect();

        // Replace secrets with redaction markers
        for secret_match in &sorted_matches {
            if let Some(line) = lines.get(secret_match.line_number - 1) {
                let (start, end) = secret_match.column_range;
                if start < line.len() && end <= line.len() {
                    let before = &line[..start];
                    let after = &line[end..];
                    let redacted_line =
                        format!("{}[REDACTED:{}]{}", before, secret_match.pattern_id, after);

                    // Replace the line in the content
                    let line_start = content
                        .lines()
                        .take(secret_match.line_number - 1)
                        .map(|l| l.len() + 1) // +1 for newline
                        .sum::<usize>();
                    let line_end = line_start + line.len();

                    redacted_content.replace_range(line_start..line_end, &redacted_line);
                }
            }
        }

        Ok(RedactionResult {
            content: redacted_content,
            matches,
            has_secrets: true,
        })
    }

    /// Check if any secrets would be detected in the content (fail-fast check)
    pub fn has_secrets(&self, content: &str, file_path: &str) -> Result<bool> {
        let matches = self.scan_for_secrets(content, file_path)?;
        Ok(!matches.is_empty())
    }

    /// Check if a pattern ID is in the ignored list
    fn is_pattern_ignored(&self, pattern_id: &str) -> bool {
        self.ignored_patterns
            .iter()
            .any(|ignored| ignored == pattern_id)
    }

    /// Find all matches for a specific pattern in content
    fn find_matches_in_content(
        &self,
        content: &str,
        file_path: &str,
        pattern_id: &str,
        regex: &Regex,
    ) -> Result<Vec<SecretMatch>> {
        let mut matches = Vec::new();

        for (line_number, line) in content.lines().enumerate() {
            for regex_match in regex.find_iter(line) {
                let start = regex_match.start();
                let end = regex_match.end();

                // Create context without revealing the secret
                let context = self.create_safe_context(line, start, end);

                matches.push(SecretMatch {
                    pattern_id: pattern_id.to_string(),
                    file_path: file_path.to_string(),
                    line_number: line_number + 1, // 1-based line numbers
                    column_range: (start, end),
                    context,
                });
            }
        }

        Ok(matches)
    }

    /// Create safe context around a match without revealing the secret
    fn create_safe_context(&self, line: &str, start: usize, end: usize) -> String {
        let before_len = 10; // Show up to 10 chars before
        let after_len = 10; // Show up to 10 chars after

        let context_start = start.saturating_sub(before_len);
        let context_end = std::cmp::min(line.len(), end + after_len);

        let before = &line[context_start..start];
        let after = &line[end..context_end];

        format!("{before}[REDACTED]{after}")
    }

    /// Get list of all pattern IDs (for configuration and logging)
    /// Extended API for pattern introspection
    #[must_use]
    #[allow(dead_code)] // Extended API for pattern introspection
    pub fn get_pattern_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        ids.extend(self.default_patterns.keys().cloned());
        ids.extend(self.extra_patterns.keys().cloned());
        ids.sort();
        ids
    }

    /// Get list of ignored pattern IDs
    /// Extended API for pattern introspection
    #[must_use]
    #[allow(dead_code)] // Extended API for pattern introspection
    pub fn get_ignored_patterns(&self) -> &[String] {
        &self.ignored_patterns
    }
}

impl Default for SecretRedactor {
    fn default() -> Self {
        Self::new().expect("Failed to create default SecretRedactor")
    }
}

static DEFAULT_REDACTOR: LazyLock<SecretRedactor> =
    LazyLock::new(|| SecretRedactor::new().expect("Failed to create default SecretRedactor"));

/// Get a process-global default redactor instance.
///
/// This is used by helpers like [`redact_user_string`] when a configured redactor
/// is not available.
#[must_use]
pub fn default_redactor() -> &'static SecretRedactor {
    &DEFAULT_REDACTOR
}

/// Global redaction function for user-facing strings
///
/// This function provides a simple way to redact secrets from any user-facing string
/// before it is displayed, logged, or persisted. It uses a default `SecretRedactor`
/// instance with all standard patterns enabled.
///
/// # Arguments
/// * `text` - The text to redact
///
/// # Returns
/// The redacted text with secrets replaced by "***"
///
/// # Example
/// ```
/// use xchecker_redaction::redact_user_string;
///
/// let token = format!("ghp_{}", "a".repeat(36));
/// let error_msg = format!("Failed to authenticate with token {token}");
/// let safe_msg = redact_user_string(&error_msg);
/// assert!(safe_msg.contains("***"));
/// assert!(!safe_msg.contains(&token));
/// ```
#[must_use]
pub fn redact_user_string(text: &str) -> String {
    default_redactor().redact_string(text)
}

/// Global redaction function for optional user-facing strings
///
/// # Arguments
/// * `text` - Optional text to redact
///
/// # Returns
/// Optional redacted text (None if input was None)
#[must_use]
#[allow(dead_code)] // Duplicate of SecretRedactor method, candidate for removal
pub fn redact_user_optional(text: &Option<String>) -> Option<String> {
    text.as_ref().map(|s| redact_user_string(s))
}

/// Global redaction function for vectors of user-facing strings
///
/// # Arguments
/// * `strings` - Vector of strings to redact
///
/// # Returns
/// Vector of redacted strings
#[must_use]
#[allow(dead_code)] // Duplicate of SecretRedactor method, candidate for removal
pub fn redact_user_strings(strings: &[String]) -> Vec<String> {
    strings.iter().map(|s| redact_user_string(s)).collect()
}

// =========================================================================
// Documentation Generation (dev-tools only)
// =========================================================================

/// Module for generating documentation from secret patterns.
///
/// This module is only available when the `dev-tools` feature is enabled.
/// It provides utilities for generating Markdown documentation from the
/// canonical pattern definitions.
#[cfg(feature = "dev-tools")]
pub mod doc_gen {
    use super::SecretPatternDef;
    use std::collections::BTreeMap;

    /// Escape pipe characters in regex patterns for markdown table cells
    #[must_use]
    pub fn escape_for_markdown(s: &str) -> String {
        s.replace('|', r"\|")
    }

    /// Normalize line endings to LF (handles Windows CRLF)
    #[must_use]
    pub fn normalize_line_endings(s: &str) -> String {
        s.replace("\r\n", "\n")
    }

    /// Render patterns grouped by category as Markdown
    ///
    /// This is the canonical rendering function used by both the documentation
    /// generator and validation tests to ensure consistency.
    #[must_use]
    pub fn render_patterns_markdown(patterns: &[SecretPatternDef]) -> String {
        // Group patterns by category using BTreeMap for consistent ordering
        let mut by_category: BTreeMap<&str, Vec<&SecretPatternDef>> = BTreeMap::new();
        for p in patterns {
            by_category.entry(p.category).or_default().push(p);
        }

        // Sort patterns within each category by ID
        for patterns in by_category.values_mut() {
            patterns.sort_by_key(|p| p.id);
        }

        // Calculate total patterns and categories
        let total_patterns = patterns.len();
        let total_categories = by_category.len();

        let mut out = String::new();

        // Header line with counts
        out.push_str(&format!(
            "xchecker includes **{} default secret patterns** across {} categories.\n",
            total_patterns, total_categories
        ));

        // Render each category
        for (category, patterns) in &by_category {
            out.push('\n');
            out.push_str(&format!(
                "#### {} ({} patterns)\n\n",
                category,
                patterns.len()
            ));
            out.push_str("| Pattern ID | Regex | Description |\n");
            out.push_str("|------------|-------|-------------|\n");
            for p in patterns {
                out.push_str(&format!(
                    "| `{}` | `{}` | {} |\n",
                    p.id,
                    escape_for_markdown(p.regex),
                    p.description
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestSecretConfig {
        extra_secret_patterns: Vec<String>,
        ignore_secret_patterns: Vec<String>,
    }

    impl SecretConfigProvider for TestSecretConfig {
        fn extra_secret_patterns(&self) -> &[String] {
            &self.extra_secret_patterns
        }

        fn ignore_secret_patterns(&self) -> &[String] {
            &self.ignore_secret_patterns
        }
    }

    impl TestSecretConfig {
        fn with_extra_patterns(mut self, patterns: Vec<String>) -> Self {
            self.extra_secret_patterns = patterns;
            self
        }

        fn with_ignore_patterns(mut self, patterns: Vec<String>) -> Self {
            self.ignore_secret_patterns = patterns;
            self
        }

        fn add_extra_pattern(mut self, pattern: &str) -> Self {
            self.extra_secret_patterns.push(pattern.to_string());
            self
        }

        fn add_ignore_pattern(mut self, pattern: &str) -> Self {
            self.ignore_secret_patterns.push(pattern.to_string());
            self
        }
    }

    #[test]
    fn test_secret_redactor_creation() {
        let redactor = SecretRedactor::new().unwrap();
        let pattern_ids = redactor.get_pattern_ids();

        // Should have all default patterns
        assert!(pattern_ids.contains(&"github_pat".to_string()));
        assert!(pattern_ids.contains(&"aws_access_key".to_string()));
        assert!(pattern_ids.contains(&"aws_secret_key".to_string()));
        assert!(pattern_ids.contains(&"slack_token".to_string()));
        assert!(pattern_ids.contains(&"bearer_token".to_string()));
    }

    #[test]
    fn test_redact_string() {
        let redactor = SecretRedactor::new().unwrap();

        // Test GitHub PAT redaction
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let text = format!("token = {}", token);
        let redacted = redactor.redact_string(&text);
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("ghp_"));

        // Test AWS key redaction
        let aws_key = "AKIAIOSFODNN7EXAMPLE";
        let text2 = format!("access_key = {}", aws_key);
        let redacted2 = redactor.redact_string(&text2);
        assert!(redacted2.contains("***"));
        assert!(!redacted2.contains("AKIA"));

        // Test no secrets
        let text3 = "This is safe text with no secrets";
        let redacted3 = redactor.redact_string(text3);
        assert_eq!(redacted3, text3);
    }

    #[test]
    fn test_redact_strings() {
        let redactor = SecretRedactor::new().unwrap();

        let github_token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let aws_key = "AKIAIOSFODNN7EXAMPLE";
        let strings = vec![
            format!("token = {}", github_token),
            "safe text".to_string(),
            format!("key = {}", aws_key),
        ];

        let redacted = redactor.redact_strings(&strings);
        assert_eq!(redacted.len(), 3);
        assert!(redacted[0].contains("***"));
        assert!(!redacted[0].contains("ghp_"));
        assert_eq!(redacted[1], "safe text");
        assert!(redacted[2].contains("***"));
        assert!(!redacted[2].contains("AKIA"));
    }

    #[test]
    fn test_redact_optional() {
        let redactor = SecretRedactor::new().unwrap();

        // Test Some with secret
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let text = Some(format!("token = {}", token));
        let redacted = redactor.redact_optional(&text);
        assert!(redacted.is_some());
        let redacted = redacted.expect("Expected redacted content");
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("ghp_"));

        // Test None
        let none_text: Option<String> = None;
        let redacted_none = redactor.redact_optional(&none_text);
        assert_eq!(redacted_none, None);
    }

    #[test]
    fn test_extra_pattern_addition() {
        let mut redactor = SecretRedactor::new().unwrap();
        redactor
            .add_extra_pattern("custom_key".to_string(), r"CUSTOM_[A-Z0-9]{10}")
            .unwrap();

        let content = "key = CUSTOM_1234567890";
        let matches = redactor.scan_for_secrets(content, "test.txt").unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_id, "custom_key");
    }

    #[test]
    fn test_pattern_ignoring() {
        let mut redactor = SecretRedactor::new().unwrap();
        redactor.add_ignored_pattern("github_pat".to_string());

        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content = format!("token = {}", token);
        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();

        // Should not detect GitHub PAT because it's ignored
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_content_redaction() {
        let redactor = SecretRedactor::new().unwrap();
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content = format!("token = {}\nother_line = safe", token);

        let result = redactor.redact_content(&content, "test.txt").unwrap();

        assert!(result.has_secrets);
        assert_eq!(result.matches.len(), 1);
        assert!(result.content.contains("[REDACTED:github_pat]"));
        assert!(!result.content.contains(token));
        assert!(result.content.contains("other_line = safe")); // Safe content preserved
    }

    #[test]
    fn test_safe_context_creation() {
        let redactor = SecretRedactor::new().unwrap();
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let line = format!("prefix_{}_suffix", token);
        let start = line.find(token).unwrap();
        let end = start + token.len();
        let context = redactor.create_safe_context(&line, start, end);

        assert!(context.contains("prefix_"));
        assert!(context.contains("[REDACTED]"));
        assert!(!context.contains(token));
    }

    #[test]
    fn test_multiple_secrets_in_content() {
        let redactor = SecretRedactor::new().unwrap();
        let github_token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let aws_key = "AKIAIOSFODNN7EXAMPLE";
        let content = format!("github_token = {}\naws_key = {}", github_token, aws_key);

        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        assert_eq!(matches.len(), 2);

        let result = redactor.redact_content(&content, "test.txt").unwrap();
        assert!(result.has_secrets);
        assert!(result.content.contains("[REDACTED:github_pat]"));
        assert!(result.content.contains("[REDACTED:aws_access_key]"));
    }

    #[test]
    fn test_huggingface_token_detection() {
        let redactor = SecretRedactor::new().unwrap();
        // Hugging Face tokens are 34 alphanumeric characters after "hf_"
        // Token: hf_ + 34 chars = 37 total
        let token = "hf_abcdefghijklmnopqrstuvwxyz12345678";
        let content = format!("export HF_TOKEN={}", token);

        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_id, "huggingface_token");

        let result = redactor.redact_content(&content, "test.txt").unwrap();
        assert!(result.has_secrets);
        assert!(result.content.contains("[REDACTED:huggingface_token]"));
        assert!(!result.content.contains(token));
    }

    #[test]
    fn test_line_number_accuracy() {
        let redactor = SecretRedactor::new().unwrap();
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content = format!("line 1\nline 2 with {}\nline 3", token);

        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line_number, 2); // Should be line 2
    }

    #[test]
    fn test_from_config_with_default_security() {
        let config = TestSecretConfig::default();
        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have all default patterns
        let pattern_ids = redactor.get_pattern_ids();
        assert!(pattern_ids.contains(&"github_pat".to_string()));
        assert!(pattern_ids.contains(&"aws_access_key".to_string()));

        // Should have no extra patterns
        assert!(
            !pattern_ids
                .iter()
                .any(|id| id.starts_with("extra_pattern_"))
        );

        // Should have no ignored patterns
        assert!(redactor.get_ignored_patterns().is_empty());
    }

    #[test]
    fn test_from_config_with_extra_patterns() {
        let config = TestSecretConfig::default().with_extra_patterns(vec![
            "CUSTOM_[A-Z0-9]{32}".to_string(),
            "MY_SECRET_[A-Za-z0-9]{20}".to_string(),
        ]);

        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have extra patterns
        let pattern_ids = redactor.get_pattern_ids();
        assert!(pattern_ids.contains(&"extra_pattern_0".to_string()));
        assert!(pattern_ids.contains(&"extra_pattern_1".to_string()));

        // Extra patterns should detect custom secrets
        let content1 = "key = CUSTOM_12345678901234567890123456789012";
        let matches1 = redactor.scan_for_secrets(content1, "test.txt").unwrap();
        assert!(!matches1.is_empty());
        assert!(matches1.iter().any(|m| m.pattern_id == "extra_pattern_0"));
    }

    #[test]
    fn test_from_config_with_ignore_patterns() {
        let config =
            TestSecretConfig::default().with_ignore_patterns(vec!["github_pat".to_string()]);

        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have ignored pattern
        assert!(
            redactor
                .get_ignored_patterns()
                .contains(&"github_pat".to_string())
        );

        // GitHub PAT should not be detected
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content = format!("token = {}", token);
        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_from_config_with_both_extra_and_ignore() {
        let config = TestSecretConfig::default()
            .with_extra_patterns(vec!["CUSTOM_[A-Z0-9]{32}".to_string()])
            .with_ignore_patterns(vec!["github_pat".to_string()]);

        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have extra pattern
        let pattern_ids = redactor.get_pattern_ids();
        assert!(pattern_ids.contains(&"extra_pattern_0".to_string()));

        // Should have ignored pattern
        assert!(
            redactor
                .get_ignored_patterns()
                .contains(&"github_pat".to_string())
        );

        // Custom secret should be detected
        let content1 = "key = CUSTOM_12345678901234567890123456789012";
        let matches1 = redactor.scan_for_secrets(content1, "test.txt").unwrap();
        assert!(!matches1.is_empty());

        // GitHub PAT should not be detected
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content2 = format!("token = {}", token);
        let matches2 = redactor.scan_for_secrets(&content2, "test.txt").unwrap();
        assert_eq!(matches2.len(), 0);
    }

    #[test]
    fn test_from_config_with_invalid_extra_pattern() {
        let config =
            TestSecretConfig::default().with_extra_patterns(vec!["[invalid regex".to_string()]);

        // Should fail to create redactor with invalid regex
        let result = SecretRedactor::from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_config_add_extra_secret_pattern_method() {
        let config = TestSecretConfig::default()
            .add_extra_pattern("SINGLE_[A-Z]{10}")
            .add_extra_pattern("ANOTHER_[0-9]{8}");

        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have both extra patterns
        let pattern_ids = redactor.get_pattern_ids();
        assert!(pattern_ids.contains(&"extra_pattern_0".to_string()));
        assert!(pattern_ids.contains(&"extra_pattern_1".to_string()));
    }

    #[test]
    fn test_from_config_add_ignore_secret_pattern_method() {
        let config = TestSecretConfig::default()
            .add_ignore_pattern("github_pat")
            .add_ignore_pattern("aws_access_key");

        let redactor = SecretRedactor::from_config(&config).unwrap();

        // Should have both ignored patterns
        let ignored = redactor.get_ignored_patterns();
        assert!(ignored.contains(&"github_pat".to_string()));
        assert!(ignored.contains(&"aws_access_key".to_string()));
    }

    #[test]
    fn test_empty_content_no_secrets() {
        let redactor = SecretRedactor::new().unwrap();
        let empty_content = "";

        // Empty content should not trigger secret detection
        assert!(!redactor.has_secrets(empty_content, "empty.txt").unwrap());

        // Scanning empty content should return no matches
        let matches = redactor
            .scan_for_secrets(empty_content, "empty.txt")
            .unwrap();
        assert!(matches.is_empty());

        // Redacting empty content should return empty content
        let result = redactor.redact_content(empty_content, "empty.txt").unwrap();
        assert_eq!(result.content, "");
        assert!(!result.has_secrets);
        assert!(result.matches.is_empty());
    }

    #[test]
    fn test_whitespace_only_content_no_secrets() {
        let redactor = SecretRedactor::new().unwrap();
        let whitespace_content = "   \n\t\n   ";

        // Whitespace-only content should not trigger secret detection
        assert!(
            !redactor
                .has_secrets(whitespace_content, "whitespace.txt")
                .unwrap()
        );

        // Redacting whitespace content should preserve it
        let result = redactor
            .redact_content(whitespace_content, "whitespace.txt")
            .unwrap();
        assert_eq!(result.content, whitespace_content);
        assert!(!result.has_secrets);
    }

    #[test]
    fn test_empty_string_redaction() {
        let empty = "";
        let redacted = redact_user_string(empty);
        assert_eq!(redacted, "");
    }

    #[test]
    fn test_empty_optional_redaction() {
        let redactor = SecretRedactor::new().unwrap();
        let none_value: Option<String> = None;
        let result = redactor.redact_optional(&none_value);
        assert_eq!(result, None);
    }

    #[test]
    fn test_empty_strings_vec_redaction() {
        let redactor = SecretRedactor::new().unwrap();
        let empty_vec: Vec<String> = vec![];
        let result = redactor.redact_strings(&empty_vec);
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_empty_file_path() {
        let redactor = SecretRedactor::new().unwrap();
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let content = format!("Some content with {}", token);

        // Empty file path should still work
        let matches = redactor.scan_for_secrets(&content, "").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, "");
    }

    #[test]
    fn test_global_redact_user_string() {
        // Test GitHub PAT
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let text = format!("Failed with token {}", token);
        let redacted = redact_user_string(&text);
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("ghp_"));

        // Test AWS key
        let aws_key = "AKIAIOSFODNN7EXAMPLE";
        let text2 = format!("Error: {} not found", aws_key);
        let redacted2 = redact_user_string(&text2);
        assert!(redacted2.contains("***"));
        assert!(!redacted2.contains("AKIA"));

        // Test Bearer token
        let bearer_token = "Bearer eyJ12345678901234567890123456789012";
        let text3 = format!("Authorization: {}", bearer_token);
        let redacted3 = redact_user_string(&text3);
        assert!(redacted3.contains("***"));
        assert!(!redacted3.contains(bearer_token));
    }

    #[test]
    fn test_global_redact_user_optional() {
        // Test Some with secret
        let token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let text = Some(format!("token = {}", token));
        let redacted = redact_user_optional(&text);
        assert!(redacted.is_some());
        let redacted = redacted.expect("Expected redacted content");
        assert!(redacted.contains("***"));
        assert!(!redacted.contains("ghp_"));

        // Test None
        let none_value: Option<String> = None;
        let redacted_none = redact_user_optional(&none_value);
        assert_eq!(redacted_none, None);
    }

    #[test]
    fn test_global_redact_user_strings() {
        let github_token = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let aws_secret =
            "AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string();
        let strings = vec![
            format!("error with {}", github_token),
            "safe message".to_string(),
            aws_secret,
        ];

        let redacted = redact_user_strings(&strings);
        assert_eq!(redacted.len(), 3);
        assert!(redacted[0].contains("***"));
        assert!(!redacted[0].contains("ghp_"));
        assert_eq!(redacted[1], "safe message");
        assert!(redacted[2].contains("***"));
        assert!(!redacted[2].contains("AWS_SECRET_ACCESS_KEY"));
    }

    #[test]
    fn test_new_infrastructure_patterns() {
        let redactor = SecretRedactor::new().unwrap();

        // Test Age Secret Key
        // Length must be exactly 58 chars after prefix
        let age_key = "AGE-SECRET-KEY-1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3jn54khab";
        let content1 = format!("age_secret = {}", age_key);
        let matches1 = redactor.scan_for_secrets(&content1, "test.txt").unwrap();
        assert_eq!(matches1.len(), 1);
        assert_eq!(matches1[0].pattern_id, "age_secret_key");

        // Test Vault Service Token
        let vault_token = "hvs.CAESIKP1234567890abcdef1234567890abcdef12345";
        let content2 = format!("vault_token = {}", vault_token);
        let matches2 = redactor.scan_for_secrets(&content2, "test.txt").unwrap();
        assert_eq!(matches2.len(), 1);
        assert_eq!(matches2[0].pattern_id, "hashicorp_vault_token");
    }

    #[test]
    fn test_all_default_patterns_exist() {
        let redactor = SecretRedactor::new().unwrap();
        let pattern_ids = redactor.get_pattern_ids();

        // AWS patterns
        assert!(pattern_ids.contains(&"aws_access_key".to_string()));
        assert!(pattern_ids.contains(&"aws_secret_key".to_string()));
        assert!(pattern_ids.contains(&"aws_session_token".to_string()));

        // GCP patterns
        assert!(pattern_ids.contains(&"gcp_api_key".to_string()));
        assert!(pattern_ids.contains(&"gcp_service_account_key".to_string()));

        // Azure patterns
        assert!(pattern_ids.contains(&"azure_storage_key".to_string()));
        assert!(pattern_ids.contains(&"azure_connection_string".to_string()));
        assert!(pattern_ids.contains(&"azure_sas_token".to_string()));

        // Generic API tokens
        assert!(pattern_ids.contains(&"bearer_token".to_string()));
        assert!(pattern_ids.contains(&"api_key_header".to_string()));
        assert!(pattern_ids.contains(&"authorization_basic".to_string()));
        assert!(pattern_ids.contains(&"oauth_token".to_string()));
        assert!(pattern_ids.contains(&"jwt_token".to_string()));

        // LLM Provider Tokens
        assert!(pattern_ids.contains(&"anthropic_api_key".to_string()));
        assert!(pattern_ids.contains(&"openai_api_key".to_string()));
        assert!(pattern_ids.contains(&"openai_legacy_key".to_string()));
        assert!(pattern_ids.contains(&"huggingface_token".to_string()));

        // Database URLs
        assert!(pattern_ids.contains(&"postgres_url".to_string()));
        assert!(pattern_ids.contains(&"mysql_url".to_string()));
        assert!(pattern_ids.contains(&"sqlserver_url".to_string()));
        assert!(pattern_ids.contains(&"mongodb_url".to_string()));
        assert!(pattern_ids.contains(&"redis_url".to_string()));

        // SSH/PEM keys
        assert!(pattern_ids.contains(&"age_secret_key".to_string()));
        assert!(pattern_ids.contains(&"ssh_private_key".to_string()));
        assert!(pattern_ids.contains(&"rsa_private_key".to_string()));
        assert!(pattern_ids.contains(&"ec_private_key".to_string()));
        assert!(pattern_ids.contains(&"pem_private_key".to_string()));
        assert!(pattern_ids.contains(&"openssh_private_key".to_string()));

        // Platform-specific tokens
        assert!(pattern_ids.contains(&"hashicorp_vault_token".to_string()));
        assert!(pattern_ids.contains(&"github_pat".to_string()));
        assert!(pattern_ids.contains(&"github_oauth".to_string()));
        assert!(pattern_ids.contains(&"github_app_token".to_string()));
        assert!(pattern_ids.contains(&"gitlab_token".to_string()));
        assert!(pattern_ids.contains(&"slack_token".to_string()));
        assert!(pattern_ids.contains(&"stripe_key".to_string()));
        assert!(pattern_ids.contains(&"twilio_key".to_string()));
        assert!(pattern_ids.contains(&"sendgrid_key".to_string()));
        assert!(pattern_ids.contains(&"npm_token".to_string()));
        assert!(pattern_ids.contains(&"pypi_token".to_string()));
        assert!(pattern_ids.contains(&"nuget_key".to_string()));
        assert!(pattern_ids.contains(&"docker_auth".to_string()));
    }
}
