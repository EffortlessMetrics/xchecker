//! Process-scoped memory tracking for benchmarking (R3.1, R3.2, R3.3, R3.4, R3.5)
//!
//! This module provides platform-specific memory measurement for the current process,
//! reporting RSS (Resident Set Size) on all platforms and commit memory on Windows.

use anyhow::Result;
use sysinfo::{Pid, System};

/// Process memory metrics
#[derive(Debug, Clone)]
pub struct ProcessMemory {
    /// Resident Set Size in MB (all platforms)
    pub rss_mb: f64,
    /// Commit memory in MB (Windows only - private bytes)
    #[cfg(target_os = "windows")]
    pub commit_mb: f64,
    /// Warning flag indicating FFI fallback was used (Windows only)
    #[cfg(target_os = "windows")]
    pub ffi_fallback: bool,
}

impl ProcessMemory {
    /// Get current process memory usage
    pub fn current() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            Self::current_windows()
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self::current_unix()
        }
    }

    /// Unix implementation using sysinfo for RSS measurement
    #[cfg(not(target_os = "windows"))]
    fn current_unix() -> Result<Self> {
        use sysinfo::ProcessesToUpdate;

        let mut sys = System::new();
        let pid = Pid::from(std::process::id() as usize);
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);

        let process = sys
            .process(pid)
            .ok_or_else(|| anyhow::anyhow!("Failed to get process information"))?;

        // sysinfo returns memory in bytes
        let rss_bytes = process.memory();
        let rss_mb = rss_bytes as f64 / (1024.0 * 1024.0);

        Ok(Self { rss_mb })
    }

    /// Windows implementation using `K32GetProcessMemoryInfo` with fallback
    #[cfg(target_os = "windows")]
    fn current_windows() -> Result<Self> {
        use std::mem;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};

        // Try FFI first
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = unsafe { mem::zeroed() };
        pmc.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;

        let success = unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&raw mut pmc).cast(),
                mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            )
        };

        if success != 0 {
            // FFI succeeded
            let rss_mb = pmc.WorkingSetSize as f64 / (1024.0 * 1024.0);
            let commit_mb = pmc.PrivateUsage as f64 / (1024.0 * 1024.0);

            Ok(Self {
                rss_mb,
                commit_mb,
                ffi_fallback: false,
            })
        } else {
            // FFI failed, fall back to sysinfo
            use sysinfo::ProcessesToUpdate;

            let mut sys = System::new();
            let pid = Pid::from(std::process::id() as usize);
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);

            let process = sys
                .process(pid)
                .ok_or_else(|| anyhow::anyhow!("Failed to get process information"))?;

            // sysinfo returns memory in bytes
            let rss_bytes = process.memory();
            let rss_mb = rss_bytes as f64 / (1024.0 * 1024.0);

            // Set commit_mb to 0.0 as fallback (we don't have this info from sysinfo)
            Ok(Self {
                rss_mb,
                commit_mb: 0.0,
                ffi_fallback: true,
            })
        }
    }

    /// Display memory usage with one decimal precision
    #[must_use]
    pub fn display(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            if self.ffi_fallback {
                format!("RSS: {:.1}MB (FFI fallback)", self.rss_mb)
            } else {
                format!("RSS: {:.1}MB, Commit: {:.1}MB", self.rss_mb, self.commit_mb)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            format!("RSS: {:.1}MB", self.rss_mb)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_memory_current() -> Result<()> {
        let mem = ProcessMemory::current()?;

        // RSS should be positive
        assert!(
            mem.rss_mb > 0.0,
            "RSS should be positive, got {}",
            mem.rss_mb
        );

        // RSS should be reasonable (less than 10GB for a test process)
        assert!(
            mem.rss_mb < 10240.0,
            "RSS should be reasonable, got {}",
            mem.rss_mb
        );

        Ok(())
    }

    #[test]
    fn test_process_scoped_not_system_wide() -> Result<()> {
        // This test verifies that we're measuring process-scoped memory, not system totals
        let mem = ProcessMemory::current()?;

        // Process RSS should be much smaller than typical system memory
        // A test process should use less than 1GB
        assert!(
            mem.rss_mb < 1024.0,
            "Process RSS should be < 1GB for a test process, got {:.1}MB. \
             This suggests we might be measuring system-wide memory instead of process-scoped.",
            mem.rss_mb
        );

        // Process RSS should be at least a few MB (reasonable for a Rust test binary)
        assert!(
            mem.rss_mb > 1.0,
            "Process RSS should be > 1MB for a Rust test binary, got {:.1}MB",
            mem.rss_mb
        );

        #[cfg(target_os = "windows")]
        {
            if !mem.ffi_fallback {
                // Commit memory should also be reasonable for a process
                assert!(
                    mem.commit_mb < 2048.0,
                    "Process commit should be < 2GB for a test process, got {:.1}MB. \
                     This suggests we might be measuring system-wide memory instead of process-scoped.",
                    mem.commit_mb
                );

                assert!(
                    mem.commit_mb > 0.0,
                    "Process commit should be positive when not using fallback, got {:.1}MB",
                    mem.commit_mb
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_display_format() -> Result<()> {
        let mem = ProcessMemory::current()?;
        let display = mem.display();

        // Should contain "RSS:" and "MB"
        assert!(
            display.contains("RSS:"),
            "Display should contain 'RSS:', got: {display}"
        );
        assert!(
            display.contains("MB"),
            "Display should contain 'MB', got: {display}"
        );

        // Should have one decimal place (check for pattern like "123.4MB")
        let has_decimal = display.chars().any(|c| c == '.');
        assert!(
            has_decimal,
            "Display should have decimal point, got: {display}"
        );

        Ok(())
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_memory_fields() -> Result<()> {
        let mem = ProcessMemory::current()?;

        // Both fields should be non-negative
        assert!(
            mem.rss_mb >= 0.0,
            "RSS should be non-negative, got {}",
            mem.rss_mb
        );
        assert!(
            mem.commit_mb >= 0.0,
            "Commit should be non-negative, got {}",
            mem.commit_mb
        );

        // If fallback is used, commit should be 0
        if mem.ffi_fallback {
            assert_eq!(mem.commit_mb, 0.0, "Commit should be 0 when using fallback");
            assert!(
                mem.display().contains("FFI fallback"),
                "Display should indicate fallback"
            );
        } else {
            // If not fallback, commit should be positive
            assert!(
                mem.commit_mb > 0.0,
                "Commit should be positive when not using fallback"
            );
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_memory_fields() -> Result<()> {
        let mem = ProcessMemory::current()?;

        // RSS should be positive
        assert!(
            mem.rss_mb > 0.0,
            "RSS should be positive, got {}",
            mem.rss_mb
        );

        // Display should not contain "Commit"
        let display = mem.display();
        assert!(
            !display.contains("Commit"),
            "Unix display should not contain 'Commit', got: {}",
            display
        );

        Ok(())
    }
}
