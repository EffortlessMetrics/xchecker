use crate::command_spec::CommandSpec;
use std::env;

use super::exec::Runner;

impl Runner {
    /// Get the WSL distro name from `wsl -l -q` or `$WSL_DISTRO_NAME`
    #[must_use]
    pub fn get_wsl_distro_name(&self) -> Option<String> {
        // First try the configured distro
        if let Some(distro) = &self.wsl_options.distro {
            return Some(distro.clone());
        }

        // Try WSL_DISTRO_NAME environment variable
        if let Ok(distro_name) = env::var("WSL_DISTRO_NAME")
            && !distro_name.is_empty()
        {
            return Some(distro_name);
        }

        // Try to get default distro using CommandSpec
        if let Ok(output) = CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
            && output.status.success()
        {
            let distros = String::from_utf8_lossy(&output.stdout);
            // Get the first non-empty line (default distro)
            for line in distros.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    return Some(line.to_string());
                }
            }
        }

        None
    }

    pub(super) fn wsl_command_spec(&self, args: &[String]) -> CommandSpec {
        // Get the claude path (default to "claude" if not specified)
        let claude_path = self.wsl_options.claude_path.as_deref().unwrap_or("claude");

        // Build WSL command: wsl.exe --exec <claude_path> <args...>
        // Use CommandSpec to ensure secure argument passing
        let mut spec = CommandSpec::new("wsl");

        // Add distro specification if provided
        if let Some(distro) = &self.wsl_options.distro {
            spec = spec.args(["-d", distro]);
        }

        spec.arg("--exec").arg(claude_path).args(args)
    }
}
