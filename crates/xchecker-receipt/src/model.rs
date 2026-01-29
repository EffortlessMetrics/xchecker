use camino::Utf8PathBuf;

use xchecker_utils::canonicalization::Canonicalizer;

/// Manages receipt creation and storage for phase execution tracking
pub struct ReceiptManager {
    pub(super) receipts_path: Utf8PathBuf,
    pub(super) canonicalizer: Canonicalizer,
}

impl ReceiptManager {
    /// Create a new `ReceiptManager` for the given spec directory
    #[must_use]
    pub fn new(spec_base_path: &Utf8PathBuf) -> Self {
        Self {
            receipts_path: spec_base_path.join("receipts"),
            canonicalizer: Canonicalizer::new(),
        }
    }
}
