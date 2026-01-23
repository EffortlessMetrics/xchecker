use anyhow::Result;
use xchecker::OrchestratorHandle;
use xchecker::artifact::{Artifact, ArtifactManager, ArtifactType};

/// Test that OrchestratorHandle rejects path traversal in spec_id
///
/// **Property: Path Traversal Rejection**
/// **Validates: Requirements FR-TEST-6**
#[test]
fn test_orchestrator_handle_spec_id_traversal() {
    // Attempt to create a handle with a spec_id containing path traversal
    // Note: OrchestratorHandle::new might succeed if it doesn't validate immediately,
    // but operations should fail or it should be sanitized.
    // Actually, if it's not sanitized, it might create directories outside specs/.

    // We use a relative path that tries to go up
    let malicious_id = "../malicious_spec";

    // If sanitization is in place, this will become ".._malicious_spec" or similar
    // If not, it might be accepted.

    // Let's check what happens.
    // We use a temporary home to avoid messing up the real environment.
    let _temp_home = xchecker::paths::with_isolated_home();

    let handle_result = OrchestratorHandle::new(malicious_id);

    // If it succeeds, we check the spec_id.
    if let Ok(handle) = handle_result {
        let id = handle.spec_id();
        // It should be sanitized.
        // sanitize_spec_id replaces '/' with '_'
        // so "../malicious_spec" -> ".._malicious_spec"
        assert_ne!(id, malicious_id, "Spec ID should have been sanitized");
        assert!(!id.contains("/"), "Spec ID should not contain slashes");
        assert!(!id.contains("\\"), "Spec ID should not contain backslashes");
    } else {
        // Failed creation is also a valid rejection (e.g. if it became empty or invalid)
    }
}

/// Test that ArtifactManager rejects path traversal in artifact names
///
/// **Property: Path Traversal Rejection**
/// **Validates: Requirements FR-TEST-6**
#[test]
fn test_artifact_manager_path_traversal() -> Result<()> {
    let _temp_home = xchecker::paths::with_isolated_home();
    let manager = ArtifactManager::new("test-spec")?;

    // Attempt to store an artifact with path traversal in the name
    let artifact = Artifact::new(
        "../../evil.md".to_string(),
        "malicious content".to_string(),
        ArtifactType::Markdown,
    );

    let result = manager.store_artifact(&artifact);

    // Should fail
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("traversal") || err.to_string().contains("parent"));

    Ok(())
}

/// Test that ArtifactManager rejects absolute paths in artifact names
///
/// **Property: Path Validation - artifact names must be relative**
/// **Validates: Requirements FR-TEST-6**
///
/// Artifact names must be relative paths. Absolute paths (e.g., "/tmp/evil.md"
/// or "C:\Windows\...") are explicitly rejected before any path prefixing occurs,
/// ensuring consistent security behavior across platforms.
#[test]
fn test_artifact_manager_absolute_path() -> Result<()> {
    let _temp_home = xchecker::paths::with_isolated_home();
    let manager = ArtifactManager::new("test-spec")?;

    // Attempt to store an artifact with absolute path
    #[cfg(unix)]
    let abs_path = "/tmp/evil.md";
    #[cfg(windows)]
    let abs_path = "C:\\Windows\\Temp\\evil.md";

    let artifact = Artifact::new(
        abs_path.to_string(),
        "malicious content".to_string(),
        ArtifactType::Markdown,
    );

    let result = manager.store_artifact(&artifact);

    // Must fail: artifact names must be relative paths
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("absolute path"),
        "Error should mention absolute path rejection: {err_msg}"
    );

    Ok(())
}
