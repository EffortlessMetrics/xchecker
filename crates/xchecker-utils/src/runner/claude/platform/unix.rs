use crate::error::RunnerError;
use std::time::Duration;

pub(crate) async fn terminate_process_unix(pid: u32) -> Result<(), RunnerError> {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let pgid = Pid::from_raw(pid as i32);

    // Send TERM signal to process group
    let _ = killpg(pgid, Signal::SIGTERM);

    // Wait up to 5 seconds for graceful termination
    let grace_period = Duration::from_secs(5);
    tokio::time::sleep(grace_period).await;

    // Send KILL signal to ensure termination
    let _ = killpg(pgid, Signal::SIGKILL);

    Ok(())
}
