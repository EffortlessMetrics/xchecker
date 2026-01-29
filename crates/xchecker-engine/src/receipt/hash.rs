use anyhow::Result;

use crate::error::XCheckerError;
use crate::types::{FileHash, FileType};

use super::ReceiptManager;

impl ReceiptManager {
    /// Create a file hash for an artifact using canonicalization
    pub fn create_file_hash(
        &self,
        file_path: &str,
        content: &str,
        file_type: FileType,
        phase: &str,
    ) -> Result<FileHash, XCheckerError> {
        let blake3_hash = self
            .canonicalizer
            .hash_canonicalized_with_context(content, file_type, phase)?;

        Ok(FileHash {
            path: file_path.to_string(),
            blake3_canonicalized: blake3_hash,
        })
    }
}
