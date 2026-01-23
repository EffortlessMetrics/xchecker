use crate::error::RunnerError;
use std::time::Duration;

pub(crate) async fn terminate_process_windows(pid: u32) -> Result<(), RunnerError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    unsafe {
        let process_handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| {
            RunnerError::NativeExecutionFailed {
                reason: format!("Failed to open process for termination: {e}"),
            }
        })?;

        // Terminate the process
        // If the process was assigned to a Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        // all child processes will be terminated when the job handle is closed
        let _ = TerminateProcess(process_handle, 1);

        // Close the handle immediately (before await)
        let _ = CloseHandle(process_handle);
    }

    // Wait a short time for graceful termination (after closing handle)
    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}
