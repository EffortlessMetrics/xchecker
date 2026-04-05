use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::atomic_write::write_file_atomic;
use crate::error::XCheckerError;
use crate::types::{PhaseId, Receipt};

use super::ReceiptManager;

static RECEIPT_FILENAME_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl ReceiptManager {
    /// Write a receipt to disk using atomic operations with JCS canonical JSON
    pub fn write_receipt(&self, receipt: &Receipt) -> Result<Utf8PathBuf> {
        // Ensure receipts directory exists (ignore benign races)
        crate::paths::ensure_dir_all(&self.receipts_path).with_context(|| {
            format!(
                "Failed to create receipts directory: {}",
                self.receipts_path
            )
        })?;

        // Generate a receipt filename using the existing second-level pattern when possible,
        // while adding a disambiguator when multiple receipts land in the same second.
        let timestamp_str = receipt.emitted_at.format("%Y%m%d_%H%M%S").to_string();
        let base_filename = format!("{}-{timestamp_str}", receipt.phase);
        let subsecond_nanos = receipt.emitted_at.timestamp_subsec_nanos();
        let filename = self.unique_receipt_filename(&base_filename, subsecond_nanos);
        let receipt_path = self.receipts_path.join(&filename);

        // Serialize receipt to canonical JSON using JCS (RFC 8785)
        let json_content = Self::emit_receipt_jcs(receipt)?;

        // Write using atomic operation (tempfile → fsync → rename)
        write_file_atomic(&receipt_path, &json_content).map_err(|e| {
            XCheckerError::ReceiptWriteFailed {
                path: receipt_path.to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(receipt_path)
    }

    /// Read the most recent receipt for a given phase
    pub fn read_latest_receipt(&self, phase: PhaseId) -> Result<Option<Receipt>> {
        let phase_str = phase.as_str();

        if !self.receipts_path.exists() {
            return Ok(None);
        }

        // Find all receipts for this phase
        let mut phase_receipts = Vec::new();

        for entry in fs::read_dir(&self.receipts_path)? {
            let entry = entry?;
            if let Some(filename) = entry.file_name().to_str()
                && filename.starts_with(&format!("{phase_str}-"))
                && filename.ends_with(".json")
            {
                phase_receipts.push(entry.path());
            }
        }

        if phase_receipts.is_empty() {
            return Ok(None);
        }

        // Sort by filename (which includes timestamp) to get the latest
        phase_receipts.sort();
        let latest_path = phase_receipts.last().unwrap();

        // Read and deserialize the latest receipt
        let content = fs::read_to_string(latest_path)
            .with_context(|| format!("Failed to read receipt: {latest_path:?}"))?;

        let receipt: Receipt = serde_json::from_str(&content)
            .with_context(|| format!("Failed to deserialize receipt: {latest_path:?}"))?;

        Ok(Some(receipt))
    }

    /// List all receipts in chronological order
    pub fn list_receipts(&self) -> Result<Vec<Receipt>> {
        if !self.receipts_path.exists() {
            return Ok(Vec::new());
        }

        let mut receipts = Vec::new();

        for entry in fs::read_dir(&self.receipts_path)? {
            let entry = entry?;
            if let Some(filename) = entry.file_name().to_str()
                && filename.ends_with(".json")
            {
                if let Ok(content) = fs::read_to_string(entry.path())
                    && let Ok(receipt) = serde_json::from_str::<Receipt>(&content)
                {
                    receipts.push(receipt);
                }
            }
        }

        // Sort by emitted_at timestamp
        receipts.sort_by(|a, b| a.emitted_at.cmp(&b.emitted_at));

        Ok(receipts)
    }

    /// Test seam; not part of public API stability guarantees.
    ///
    /// Get the path to the receipts directory.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub const fn receipts_path(&self) -> &Utf8PathBuf {
        &self.receipts_path
    }

    fn unique_receipt_filename(&self, base_filename: &str, subsecond_nanos: u32) -> String {
        let candidate = if subsecond_nanos > 0 {
            format!("{base_filename}-{subsecond_nanos:09}.json")
        } else {
            format!("{base_filename}.json")
        };

        if !self.receipts_path.join(&candidate).exists() {
            return candidate;
        }

        let mut sequence = RECEIPT_FILENAME_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;

        loop {
            let candidate = if subsecond_nanos > 0 {
                format!("{base_filename}-{subsecond_nanos:09}-{sequence:09}.json")
            } else {
                format!("{base_filename}-{sequence:09}.json")
            };

            if !self.receipts_path.join(&candidate).exists() {
                return candidate;
            }

            sequence += 1;
        }
    }
}

/// Helper function to append `rename_retry_count` to receipt warnings
///
/// This function is used on Windows when atomic rename operations require
/// retry backoff due to transient filesystem issues.
///
/// # Arguments
///
/// * `warnings` - Mutable reference to the warnings vector
/// * `retry_count` - Optional retry count (None if no retries occurred)
///
/// # Examples
///
/// ```
/// let mut warnings = vec![];
/// xchecker_engine::receipt::add_rename_retry_warning(&mut warnings, Some(3));
/// assert_eq!(warnings.len(), 1);
/// assert_eq!(warnings[0], "rename_retry_count: 3");
///
/// let mut warnings2 = vec![];
/// xchecker_engine::receipt::add_rename_retry_warning(&mut warnings2, None);
/// assert_eq!(warnings2.len(), 0);
/// ```
#[allow(dead_code)] // Receipt utility for tracking atomic write retries
pub fn add_rename_retry_warning(warnings: &mut Vec<String>, retry_count: Option<u32>) {
    if let Some(count) = retry_count {
        warnings.push(format!("rename_retry_count: {count}"));
    }
}
