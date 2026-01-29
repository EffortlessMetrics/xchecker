use crate::command_spec::CommandSpec;
use std::ffi::OsString;
use std::path::Path;

use super::exec::Runner;

impl Runner {
    pub(super) fn native_command_spec(&self, args: &[String]) -> CommandSpec {
        let (program, base_args) = self.resolve_native_command();
        let mut spec = CommandSpec::new(program);
        if !base_args.is_empty() {
            spec = spec.args(base_args);
        }
        spec.args(args)
    }

    fn resolve_native_command(&self) -> (OsString, Vec<OsString>) {
        let Some(path) = self.wsl_options.claude_path.as_deref() else {
            return (OsString::from("claude"), Vec::new());
        };

        let trimmed = path.trim();
        if trimmed.is_empty() {
            return (OsString::from("claude"), Vec::new());
        }

        if Path::new(trimmed).exists() {
            return (OsString::from(trimmed), Vec::new());
        }

        if trimmed.chars().any(char::is_whitespace) {
            let parts = Self::split_command_line(trimmed);
            if let Some((program, rest)) = parts.split_first() {
                let base_args = rest.iter().cloned().map(OsString::from).collect::<Vec<_>>();
                return (OsString::from(program), base_args);
            }
        }

        (OsString::from(trimmed), Vec::new())
    }

    fn split_command_line(input: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = input.chars().peekable();
        let mut in_single = false;
        let mut in_double = false;

        while let Some(ch) = chars.next() {
            match ch {
                '\'' if !in_double => {
                    in_single = !in_single;
                }
                '"' if !in_single => {
                    in_double = !in_double;
                }
                '\\' if in_double => {
                    if let Some(&next) = chars.peek() {
                        if next == '"' {
                            chars.next();
                            current.push('"');
                        } else {
                            current.push('\\');
                        }
                    } else {
                        current.push('\\');
                    }
                }
                c if c.is_whitespace() && !in_single && !in_double => {
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }
}

#[cfg(test)]
mod tests {
    use super::Runner;

    #[test]
    fn split_command_line_preserves_backslashes_in_quotes() {
        let input = r#""C:\Program Files\Claude\claude.exe" --flag"#;
        let parts = Runner::split_command_line(input);
        assert_eq!(
            parts,
            vec!["C:\\Program Files\\Claude\\claude.exe", "--flag"]
        );
    }

    #[test]
    fn split_command_line_preserves_double_backslash() {
        let input = r#""C:\\Temp\\Claude\\claude.exe""#;
        let parts = Runner::split_command_line(input);
        assert_eq!(parts, vec![r#"C:\\Temp\\Claude\\claude.exe"#]);
    }

    #[test]
    fn split_command_line_allows_escaped_quotes() {
        let input = r#"--arg "value with \"quote\"""#;
        let parts = Runner::split_command_line(input);
        assert_eq!(parts, vec!["--arg", r#"value with "quote""#]);
    }
}
