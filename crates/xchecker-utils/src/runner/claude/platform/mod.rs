use crate::error::RunnerError;
use std::time::Duration;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_job;

#[cfg(windows)]
pub(crate) use windows_job::{assign_to_job, create_job_object};

pub(crate) async fn terminate_process_by_pid(
    pid: u32,
    _timeout_duration: Duration,
) -> Result<(), RunnerError> {
    #[cfg(unix)]
    {
        unix::terminate_process_unix(pid).await
    }

    #[cfg(windows)]
    {
        windows::terminate_process_windows(pid).await
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn test_job_objects_not_available_on_non_windows() {
        // Job Objects are Windows-only, so this test just verifies
        // that the code compiles on non-Windows platforms
        println!("Job Object tests are Windows-only");
    }
}
