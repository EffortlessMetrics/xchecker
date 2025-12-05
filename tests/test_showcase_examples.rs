//! Tests for showcase examples
//!
//! These tests validate that the showcase examples in the `examples/` directory
//! have the correct structure and configuration.
//!
//! Requirements:
//! - 4.9.1: examples/fullstack-nextjs/ with working scenario and README
//! - 4.9.2: examples/mono-repo/ demonstrating multiple specs under one workspace

use std::path::Path;

/// Validate that a file exists at the given path
fn assert_file_exists(path: &str) {
    let p = Path::new(path);
    assert!(
        p.exists() && p.is_file(),
        "Expected file to exist: {}",
        path
    );
}

/// Validate that a directory exists at the given path
fn assert_dir_exists(path: &str) {
    let p = Path::new(path);
    assert!(
        p.exists() && p.is_dir(),
        "Expected directory to exist: {}",
        path
    );
}

/// Validate that a file contains expected content
fn assert_file_contains(path: &str, expected: &str) {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read file: {}", path));
    assert!(
        content.contains(expected),
        "File {} should contain '{}'\nActual content:\n{}",
        path,
        expected,
        &content[..content.len().min(500)]
    );
}

// ============================================================================
// fullstack-nextjs example tests
// ============================================================================

#[test]
fn test_fullstack_nextjs_readme_exists() {
    assert_file_exists("examples/fullstack-nextjs/README.md");
}

#[test]
fn test_fullstack_nextjs_readme_content() {
    assert_file_contains(
        "examples/fullstack-nextjs/README.md",
        "Full-Stack Next.js Example",
    );
    assert_file_contains(
        "examples/fullstack-nextjs/README.md",
        "xchecker",
    );
}

#[test]
fn test_fullstack_nextjs_workspace_exists() {
    assert_file_exists("examples/fullstack-nextjs/workspace.yaml");
}

#[test]
fn test_fullstack_nextjs_workspace_content() {
    assert_file_contains(
        "examples/fullstack-nextjs/workspace.yaml",
        "fullstack-nextjs-example",
    );
    assert_file_contains(
        "examples/fullstack-nextjs/workspace.yaml",
        "task-manager",
    );
}

#[test]
fn test_fullstack_nextjs_config_exists() {
    assert_file_exists("examples/fullstack-nextjs/.xchecker/config.toml");
}

#[test]
fn test_fullstack_nextjs_config_content() {
    assert_file_contains(
        "examples/fullstack-nextjs/.xchecker/config.toml",
        "[defaults]",
    );
    assert_file_contains(
        "examples/fullstack-nextjs/.xchecker/config.toml",
        "[selectors]",
    );
}

#[test]
fn test_fullstack_nextjs_spec_structure() {
    assert_dir_exists("examples/fullstack-nextjs/.xchecker/specs/task-manager");
    assert_dir_exists("examples/fullstack-nextjs/.xchecker/specs/task-manager/context");
    assert_file_exists(
        "examples/fullstack-nextjs/.xchecker/specs/task-manager/context/problem-statement.md",
    );
}

#[test]
fn test_fullstack_nextjs_problem_statement_content() {
    assert_file_contains(
        "examples/fullstack-nextjs/.xchecker/specs/task-manager/context/problem-statement.md",
        "Task Manager",
    );
    assert_file_contains(
        "examples/fullstack-nextjs/.xchecker/specs/task-manager/context/problem-statement.md",
        "Next.js",
    );
}

#[test]
fn test_fullstack_nextjs_validation_scripts_exist() {
    assert_file_exists("examples/fullstack-nextjs/scripts/validate.sh");
    assert_file_exists("examples/fullstack-nextjs/scripts/validate.ps1");
}

// ============================================================================
// mono-repo example tests
// ============================================================================

#[test]
fn test_mono_repo_readme_exists() {
    assert_file_exists("examples/mono-repo/README.md");
}

#[test]
fn test_mono_repo_readme_content() {
    assert_file_contains("examples/mono-repo/README.md", "Mono-Repo Example");
    assert_file_contains("examples/mono-repo/README.md", "multiple specs");
}

#[test]
fn test_mono_repo_workspace_exists() {
    assert_file_exists("examples/mono-repo/workspace.yaml");
}

#[test]
fn test_mono_repo_workspace_content() {
    let content = std::fs::read_to_string("examples/mono-repo/workspace.yaml")
        .expect("Failed to read workspace.yaml");
    
    // Verify all three specs are registered
    assert!(content.contains("user-service"), "Should contain user-service spec");
    assert!(content.contains("product-catalog"), "Should contain product-catalog spec");
    assert!(content.contains("order-api"), "Should contain order-api spec");
    
    // Verify tags are present
    assert!(content.contains("rust"), "Should have rust tag");
    assert!(content.contains("python"), "Should have python tag");
    assert!(content.contains("backend"), "Should have backend tag");
}

#[test]
fn test_mono_repo_config_exists() {
    assert_file_exists("examples/mono-repo/.xchecker/config.toml");
}

#[test]
fn test_mono_repo_config_content() {
    assert_file_contains("examples/mono-repo/.xchecker/config.toml", "[defaults]");
    assert_file_contains("examples/mono-repo/.xchecker/config.toml", "[selectors]");
    // Should include patterns for both Rust and Python
    assert_file_contains("examples/mono-repo/.xchecker/config.toml", "*.rs");
    assert_file_contains("examples/mono-repo/.xchecker/config.toml", "*.py");
}

#[test]
fn test_mono_repo_user_service_spec() {
    assert_dir_exists("examples/mono-repo/.xchecker/specs/user-service");
    assert_dir_exists("examples/mono-repo/.xchecker/specs/user-service/context");
    assert_file_exists(
        "examples/mono-repo/.xchecker/specs/user-service/context/problem-statement.md",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/user-service/context/problem-statement.md",
        "User Service",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/user-service/context/problem-statement.md",
        "authentication",
    );
}

#[test]
fn test_mono_repo_product_catalog_spec() {
    assert_dir_exists("examples/mono-repo/.xchecker/specs/product-catalog");
    assert_dir_exists("examples/mono-repo/.xchecker/specs/product-catalog/context");
    assert_file_exists(
        "examples/mono-repo/.xchecker/specs/product-catalog/context/problem-statement.md",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/product-catalog/context/problem-statement.md",
        "Product Catalog",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/product-catalog/context/problem-statement.md",
        "FastAPI",
    );
}

#[test]
fn test_mono_repo_order_api_spec() {
    assert_dir_exists("examples/mono-repo/.xchecker/specs/order-api");
    assert_dir_exists("examples/mono-repo/.xchecker/specs/order-api/context");
    assert_file_exists(
        "examples/mono-repo/.xchecker/specs/order-api/context/problem-statement.md",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/order-api/context/problem-statement.md",
        "Order API",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/specs/order-api/context/problem-statement.md",
        "order processing",
    );
}

#[test]
fn test_mono_repo_validation_scripts_exist() {
    assert_file_exists("examples/mono-repo/scripts/validate.sh");
    assert_file_exists("examples/mono-repo/scripts/validate.ps1");
}

// ============================================================================
// Cross-example validation
// ============================================================================

#[test]
fn test_examples_use_consistent_schema_version() {
    // Both workspaces should use version "1"
    assert_file_contains("examples/fullstack-nextjs/workspace.yaml", "version: \"1\"");
    assert_file_contains("examples/mono-repo/workspace.yaml", "version: \"1\"");
}

#[test]
fn test_examples_use_consistent_llm_config() {
    // Both configs should use claude-cli provider
    assert_file_contains(
        "examples/fullstack-nextjs/.xchecker/config.toml",
        "provider = \"claude-cli\"",
    );
    assert_file_contains(
        "examples/mono-repo/.xchecker/config.toml",
        "provider = \"claude-cli\"",
    );
}
