use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;

// ============================================================================
// CommandSpec - Secure Process Execution Specification
// ============================================================================

/// Specification for a command to execute.
///
/// All process execution goes through this type to ensure argv-style invocation.
/// This prevents shell injection attacks by ensuring arguments are passed as
/// discrete elements rather than shell strings.
///
/// # Security
///
/// `CommandSpec` enforces that:
/// - Arguments are `Vec<OsString>`, NOT shell strings
/// - No shell string evaluation (`sh -c`, `cmd /C`) is used
/// - Arguments cross trust boundaries as discrete elements
///
/// # Example
///
/// ```rust
/// use xchecker_utils::runner::CommandSpec;
/// use std::ffi::OsString;
///
/// let cmd = CommandSpec::new("claude")
///     .arg("--print")
///     .arg("--output-format")
///     .arg("json")
///     .cwd("/path/to/workspace");
///
/// assert_eq!(cmd.program, OsString::from("claude"));
/// assert_eq!(cmd.args.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// The program to execute
    pub program: OsString,
    /// Arguments as discrete elements (NOT shell strings)
    pub args: Vec<OsString>,
    /// Optional working directory
    pub cwd: Option<PathBuf>,
    /// Optional environment overrides
    pub env: Option<HashMap<OsString, OsString>>,
}

impl CommandSpec {
    /// Create a new `CommandSpec` with the given program.
    ///
    /// # Arguments
    ///
    /// * `program` - The program to execute. Can be any type that converts to `OsString`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude");
    /// ```
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: None,
        }
    }

    /// Add a single argument to the command.
    ///
    /// Arguments are stored as discrete `OsString` elements, ensuring no shell
    /// interpretation occurs.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to add. Can be any type that converts to `OsString`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .arg("--print")
    ///     .arg("--verbose");
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments to the command.
    ///
    /// Arguments are stored as discrete `OsString` elements, ensuring no shell
    /// interpretation occurs.
    ///
    /// # Arguments
    ///
    /// * `args` - An iterator of arguments to add.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .args(["--print", "--output-format", "json"]);
    /// ```
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Set the working directory for the command.
    ///
    /// # Arguments
    ///
    /// * `cwd` - The working directory path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .cwd("/path/to/workspace");
    /// ```
    #[must_use]
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set an environment variable for the command.
    ///
    /// # Arguments
    ///
    /// * `key` - The environment variable name.
    /// * `value` - The environment variable value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .env("CLAUDE_API_KEY", "sk-...")
    ///     .env("DEBUG", "1");
    /// ```
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables for the command.
    ///
    /// # Arguments
    ///
    /// * `envs` - An iterator of (key, value) pairs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .envs([("DEBUG", "1"), ("VERBOSE", "true")]);
    /// ```
    #[must_use]
    pub fn envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let env_map = self.env.get_or_insert_with(HashMap::new);
        for (key, value) in envs {
            env_map.insert(key.into(), value.into());
        }
        self
    }

    /// Convert this `CommandSpec` into a `std::process::Command`.
    ///
    /// This is the primary way to execute a `CommandSpec`. The resulting `Command`
    /// uses argv-style argument passing, ensuring no shell injection is possible.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("echo")
    ///     .arg("hello")
    ///     .arg("world");
    ///
    /// let output = cmd.to_command().output().expect("failed to execute");
    /// ```
    #[must_use]
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env) = self.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        cmd
    }

    /// Convert this `CommandSpec` into a `tokio::process::Command`.
    ///
    /// This is used for async execution with timeout support.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_utils::runner::CommandSpec;
    ///
    /// # async fn example() {
    /// let cmd = CommandSpec::new("echo")
    ///     .arg("hello");
    ///
    /// let output = cmd.to_tokio_command().output().await.expect("failed to execute");
    /// # }
    /// ```
    #[must_use]
    pub fn to_tokio_command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new(&self.program);
        cmd.args(&self.args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env) = self.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        cmd
    }
}

impl Default for CommandSpec {
    fn default() -> Self {
        Self {
            program: OsString::new(),
            args: Vec::new(),
            cwd: None,
            env: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_command_spec_new() {
        let cmd = CommandSpec::new("claude");
        assert_eq!(cmd.program, OsString::from("claude"));
        assert!(cmd.args.is_empty());
        assert!(cmd.cwd.is_none());
        assert!(cmd.env.is_none());
    }

    #[test]
    fn test_command_spec_arg() {
        let cmd = CommandSpec::new("claude").arg("--print").arg("--verbose");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.args[0], OsString::from("--print"));
        assert_eq!(cmd.args[1], OsString::from("--verbose"));
    }

    #[test]
    fn test_command_spec_args() {
        let cmd = CommandSpec::new("claude").args(["--print", "--output-format", "json"]);
        assert_eq!(cmd.args.len(), 3);
        assert_eq!(cmd.args[0], OsString::from("--print"));
        assert_eq!(cmd.args[1], OsString::from("--output-format"));
        assert_eq!(cmd.args[2], OsString::from("json"));
    }

    #[test]
    fn test_command_spec_cwd() {
        let cmd = CommandSpec::new("claude").cwd("/path/to/workspace");
        assert_eq!(cmd.cwd, Some(PathBuf::from("/path/to/workspace")));
    }

    #[test]
    fn test_command_spec_env() {
        let cmd = CommandSpec::new("claude")
            .env("DEBUG", "1")
            .env("VERBOSE", "true");
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(
            env.get(&OsString::from("DEBUG")),
            Some(&OsString::from("1"))
        );
        assert_eq!(
            env.get(&OsString::from("VERBOSE")),
            Some(&OsString::from("true"))
        );
    }

    #[test]
    fn test_command_spec_envs() {
        let cmd = CommandSpec::new("claude").envs([("DEBUG", "1"), ("VERBOSE", "true")]);
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(
            env.get(&OsString::from("DEBUG")),
            Some(&OsString::from("1"))
        );
        assert_eq!(
            env.get(&OsString::from("VERBOSE")),
            Some(&OsString::from("true"))
        );
    }

    #[test]
    fn test_command_spec_builder_chain() {
        let cmd = CommandSpec::new("claude")
            .arg("--print")
            .args(["--output-format", "json"])
            .cwd("/workspace")
            .env("DEBUG", "1")
            .envs([("VERBOSE", "true")]);

        assert_eq!(cmd.program, OsString::from("claude"));
        assert_eq!(cmd.args.len(), 3);
        assert_eq!(cmd.cwd, Some(PathBuf::from("/workspace")));
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
    }

    #[test]
    fn test_command_spec_default() {
        let cmd = CommandSpec::default();
        assert_eq!(cmd.program, OsString::new());
        assert!(cmd.args.is_empty());
        assert!(cmd.cwd.is_none());
        assert!(cmd.env.is_none());
    }

    #[test]
    fn test_command_spec_clone() {
        let cmd = CommandSpec::new("claude")
            .arg("--print")
            .cwd("/workspace")
            .env("DEBUG", "1");
        let cloned = cmd.clone();

        assert_eq!(cloned.program, cmd.program);
        assert_eq!(cloned.args, cmd.args);
        assert_eq!(cloned.cwd, cmd.cwd);
        assert_eq!(cloned.env, cmd.env);
    }

    #[test]
    fn test_command_spec_to_command() {
        let cmd = CommandSpec::new("echo").arg("hello").arg("world");

        // Verify we can create a std::process::Command
        let std_cmd = cmd.to_command();
        // We can't easily inspect the Command, but we can verify it doesn't panic
        assert!(std::mem::size_of_val(&std_cmd) > 0);
    }

    #[test]
    fn test_command_spec_to_tokio_command() {
        let cmd = CommandSpec::new("echo").arg("hello");

        // Verify we can create a tokio::process::Command
        let tokio_cmd = cmd.to_tokio_command();
        // We can't easily inspect the Command, but we can verify it doesn't panic
        assert!(std::mem::size_of_val(&tokio_cmd) > 0);
    }

    #[test]
    fn test_command_spec_osstring_args() {
        // Test that we can use OsString directly
        let cmd = CommandSpec::new(OsString::from("claude")).arg(OsString::from("--print"));
        assert_eq!(cmd.program, OsString::from("claude"));
        assert_eq!(cmd.args[0], OsString::from("--print"));
    }

    #[test]
    fn test_command_spec_args_are_vec_osstring() {
        // Verify args are stored as Vec<OsString>, not shell strings
        let cmd = CommandSpec::new("claude")
            .arg("arg with spaces")
            .arg("arg;with;semicolons")
            .arg("arg|with|pipes")
            .arg("arg&with&ampersands");

        // Each argument should be stored as a discrete OsString element
        assert_eq!(cmd.args.len(), 4);
        assert_eq!(cmd.args[0], OsString::from("arg with spaces"));
        assert_eq!(cmd.args[1], OsString::from("arg;with;semicolons"));
        assert_eq!(cmd.args[2], OsString::from("arg|with|pipes"));
        assert_eq!(cmd.args[3], OsString::from("arg&with&ampersands"));
    }

    #[test]
    fn test_command_spec_shell_metacharacters_preserved() {
        // Verify that shell metacharacters are preserved as-is (not interpreted)
        // This is critical for security - we don't want shell injection
        let cmd = CommandSpec::new("echo")
            .arg("$(whoami)")
            .arg("`id`")
            .arg("${HOME}")
            .arg("$PATH");

        // These should be stored literally, not expanded
        assert_eq!(cmd.args[0], OsString::from("$(whoami)"));
        assert_eq!(cmd.args[1], OsString::from("`id`"));
        assert_eq!(cmd.args[2], OsString::from("${HOME}"));
        assert_eq!(cmd.args[3], OsString::from("$PATH"));
    }
}
