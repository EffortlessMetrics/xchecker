use anyhow::Result;
use xchecker::artifact::{Artifact, ArtifactManager, ArtifactType};
use xchecker::OrchestratorHandle;

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
/// **Property: Path Traversal Rejection**
/// **Validates: Requirements FR-TEST-6**
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
    
    // Should fail
    // Note: ArtifactManager prepends "artifacts/", so absolute paths become relative paths
    // (e.g. "artifacts//tmp/evil.md" or "artifacts/C:\...").
    // On Windows, "artifacts/C:\..." is an invalid path, so write fails.
    // On Unix, "artifacts//tmp/evil.md" is valid but inside artifacts/, so it's safe.
    // However, SandboxRoot might reject it if it detects absolute path components?
    // SandboxRoot::join rejects absolute paths.
    // But "artifacts/..." is not absolute.
    // So the failure comes from filesystem or other validation.
    // In any case, it should not succeed in writing to the absolute path.
    assert!(result.is_err());
    
    Ok(())
}
