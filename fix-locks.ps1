# Script to fix xchecker-lock imports
$content = Get-Content "crates\xchecker-lock\src\lib.rs" -Raw
# Replace all crate:: imports with xchecker_utils::
$content = $content -replace "crate::atomic_write", "xchecker_utils::atomic_write"
$content = $content -replace "crate::error::LockError", "xchecker_utils::error::LockError"
$content = $content -replace "crate::types::{DriftPair, LockDrift}", "xchecker_utils::types::{DriftPair, LockDrift}"
$content = $content -replace "crate::paths::spec_root", "xchecker_utils::paths::spec_root"
$content = $content -replace "crate::paths::ensure_dir_all", "xchecker_utils::paths::ensure_dir_all"
$content = $content -replace "crate::paths::with_isolated_home", "xchecker_utils::paths::with_isolated_home"
$content = $content -replace "crate::chrono::{DateTime, Utc}", "chrono::{DateTime, Utc}"
$content = $content -replace "crate::camino::Utf8PathBuf", "camino::Utf8PathBuf"
$content = $content -replace "crate::fd_lock::RwLock", "fd_lock::RwLock"
$content = $content -replace "crate::serde::{Deserialize, Serialize}", "serde::{Deserialize, Serialize}"
$content = $content -replace "crate::serde_json::from_str", "serde_json::from_str"
$content = $content -replace "crate::serde_json::to_string_pretty", "serde_json::to_string_pretty"
$content = $content -replace "crate::anyhow::Result", "anyhow::Result"
$content = $content -replace "crate::std::fs", "std::fs"
$content = $content -replace "crate::std::io::{self, Write}", "std::io::{self, Write}"
$content = $content -replace "crate::std::path::{Path, PathBuf}", "std::path::{Path, PathBuf}"
$content = $content -replace "crate::std::process", "std::process"
$content = $content -replace "crate::std::time::{SystemTime, UNIX_EPOCH}", "std::time::{SystemTime, UNIX_EPOCH}"
$content = $content -replace "crate::winapi::um::handleapi::CloseHandle", "winapi::um::handleapi::CloseHandle"
$content = $content -replace "crate::winapi::um::minwinbase::STILL_ACTIVE", "winapi::um::minwinbase::STILL_ACTIVE"
$content = $content -replace "crate::winapi::um::processthreadsapi::{GetExitCodeProcess, OpenProcess}", "winapi::um::processthreadsapi::{GetExitCodeProcess, OpenProcess}"
$content = $content -replace "crate::winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION", "winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION"

# Write the fixed content
Set-Content -Path "crates\xchecker-lock\src\lib.rs" -Value $content
