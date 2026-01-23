use crate::error::RunnerError;

/// RAII wrapper for Windows Job Object handle
///
/// Ensures that the Job Object handle is properly closed when dropped,
/// which triggers termination of all processes in the job (due to `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
pub(crate) struct JobObjectHandle {
    handle: windows::Win32::Foundation::HANDLE,
}

// SAFETY: Windows HANDLEs are safe to send between threads.
// The HANDLE is an opaque kernel object reference that can be used from any thread.
unsafe impl Send for JobObjectHandle {}

impl Drop for JobObjectHandle {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Create a Windows Job Object for process tree termination
///
/// Creates a Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag,
/// which ensures that all processes in the job are terminated when the job handle is closed.
/// This provides reliable process tree termination on Windows.
pub(crate) fn create_job_object() -> Result<JobObjectHandle, RunnerError> {
    use windows::Win32::System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    unsafe {
        let job = CreateJobObjectW(None, None).map_err(|e| RunnerError::NativeExecutionFailed {
            reason: format!("Failed to create Job Object: {e}"),
        })?;

        // Configure job to kill all processes when closed
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|e| RunnerError::NativeExecutionFailed {
            reason: format!("Failed to configure Job Object: {e}"),
        })?;

        Ok(JobObjectHandle { handle: job })
    }
}

/// Assign a process to a Windows Job Object
///
/// Assigns the given process to the Job Object, ensuring that when the job is closed,
/// this process and all its children will be terminated.
pub(crate) fn assign_to_job(
    job: &JobObjectHandle,
    child: &tokio::process::Child,
) -> Result<(), RunnerError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

    if let Some(pid) = child.id() {
        unsafe {
            let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).map_err(|e| {
                RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to open process for job assignment: {e}"),
                }
            })?;

            AssignProcessToJobObject(job.handle, process_handle).map_err(|e| {
                // Close the process handle before returning error
                let _ = CloseHandle(process_handle);
                RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to assign process to Job Object: {e}"),
                }
            })?;

            // Close the process handle after successful assignment
            let _ = CloseHandle(process_handle);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::create_job_object;

    #[test]
    fn test_job_object_handle_creation() {
        // Test that we can create a Job Object handle
        let result = create_job_object();
        assert!(result.is_ok(), "Job Object creation should succeed");

        // The handle should be dropped automatically when it goes out of scope
        println!("Job Object handle creation test passed");
    }

    #[test]
    fn test_job_object_handle_drop() {
        // Test that the RAII wrapper properly drops the handle
        {
            let _job = create_job_object().unwrap();
            // Job handle should be valid here
        }
        // Job handle should be closed here

        println!("Job Object handle drop test passed");
    }

    #[test]
    fn test_multiple_job_object_handles() {
        // Test that we can create multiple Job Object handles
        let job1 = create_job_object();
        let job2 = create_job_object();

        assert!(job1.is_ok(), "First Job Object creation should succeed");
        assert!(job2.is_ok(), "Second Job Object creation should succeed");

        println!("Multiple Job Object handles test passed");
    }
}
