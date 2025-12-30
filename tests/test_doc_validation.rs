#![cfg(feature = "dev-tools")]
//! Documentation validation integration tests
//!
//! This test suite verifies that all xchecker documentation is accurate and
//! aligned with the current implementation. It validates:
//!
//! - README commands, options, and exit codes (R1)
//! - Schema examples validity and completeness (R2)
//! - Configuration documentation accuracy (R3)
//! - Doctor documentation correctness (R4)
//! - Contracts documentation accuracy (R5)
//! - Schema-Rust struct conformance (R6)
//! - CHANGELOG completeness (R7)
//! - `XCHECKER_HOME` documentation (R8)
//! - Code example execution (R9)
//! - Feature documentation accuracy (R10)
//!
//! Run with: cargo test --features dev-tools --test doc_validation

mod doc_validation;
