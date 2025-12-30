use anyhow::Result;
use tempfile::TempDir;
use xchecker::artifact::{Artifact, ArtifactType};
use xchecker::fixup::{FixupMode, FixupParser, UnifiedDiff, DiffHunk};
use xchecker::OrchestratorHandle;

/// Test environment setup
struct SecurityTestEnvironment {
    _temp_dir: TempDir,
    handle: OrchestratorHandle,
}

impl SecurityTestEnvironment {
    fn new(test_name: &str) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        std::env::set_current_dir(temp_dir.path())?;

        // Create .xchecker directory structure
        std::fs::create_dir_all(temp_dir.path().join(".xchecker/specs"))?;

        let spec_id = format!("security-{test_name}");
        let handle = OrchestratorHandle::new(&spec_id)?;

        Ok(Self {
            _temp_dir: temp_dir,
            handle,
        })
    }
}

/// Test 1: Artifact Path Traversal via OrchestratorHandle -> ArtifactManager
/// Validates that ArtifactManager rejects paths attempting to escape the spec directory
/// Requirements: FR-TEST-6
#[test]
fn test_artifact_path_traversal_rejection() -> Result<()> {
    let env = SecurityTestEnvironment::new("artifact-traversal")?;
    let manager = env.handle.artifact_manager();

    // Attempt 1: Parent directory traversal
    let malicious_artifact = Artifact::new(
        "../evil.md".to_string(),
        "malicious content".to_string(),
        ArtifactType::Markdown,
    );

    let result = manager.store_artifact(&malicious_artifact);
    assert!(result.is_err(), "Should reject parent directory traversal");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("traversal") || err.to_string().contains("parent"),
        "Error should mention traversal or parent directory"
    );

    Ok(())
}

/// Test 2: Fixup Path Traversal via FixupParser (initialized from OrchestratorHandle context)
/// Validates that FixupParser rejects diffs targeting files outside the repo
/// Requirements: FR-TEST-6
#[test]
fn test_fixup_path_traversal_rejection() -> Result<()> {
    let env = SecurityTestEnvironment::new("fixup-traversal")?;
    
    // Get the base path from the artifact manager to initialize FixupParser
    // This simulates the environment the FixupPhase would run in
    let base_path = env.handle.artifact_manager().base_path().to_path_buf();
    
    // Initialize FixupParser in Apply mode
    let parser = FixupParser::new(FixupMode::Apply, base_path.into_std_path_buf())
        .expect("Failed to create FixupParser");

    // Case 1: Parent directory traversal
    let traversal_diff = UnifiedDiff {
        target_file: "../evil.txt".to_string(),
        diff_content: "diff --git a/../evil.txt b/../evil.txt\n...".to_string(),
        hunks: vec![DiffHunk {
            old_range: (1, 1),
            new_range: (1, 1),
            content: "@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
        }],
    };

    let result_traversal = parser.apply_changes(&[traversal_diff]);
    assert!(result_traversal.is_ok());
    let fixup_result_traversal = result_traversal.unwrap();
    
    assert!(fixup_result_traversal.applied_files.is_empty());
    assert_eq!(fixup_result_traversal.failed_files.len(), 1);
    assert_eq!(fixup_result_traversal.failed_files[0], "../evil.txt");
    
    let warnings_traversal = fixup_result_traversal.warnings.join("\n");
    assert!(
        warnings_traversal.contains("Path validation failed") || warnings_traversal.contains("traversal") || warnings_traversal.contains("parent"),
        "Warnings should mention path validation failure: {}", warnings_traversal
    );

    // Case 2: Absolute path
    #[cfg(unix)]
    let abs_path = "/tmp/evil.txt";
    #[cfg(windows)]
    let abs_path = "C:\\Windows\\Temp\\evil.txt";

    let abs_diff = UnifiedDiff {
        target_file: abs_path.to_string(),
        diff_content: format!("diff --git a{} b{}\n...", abs_path, abs_path),
        hunks: vec![DiffHunk {
            old_range: (1, 1),
            new_range: (1, 1),
            content: "@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
        }],
    };

    let result_abs = parser.apply_changes(&[abs_diff]);
    assert!(result_abs.is_ok());
    let fixup_result_abs = result_abs.unwrap();

    assert!(fixup_result_abs.applied_files.is_empty());
    assert_eq!(fixup_result_abs.failed_files.len(), 1);
    assert_eq!(fixup_result_abs.failed_files[0], abs_path);

    let warnings_abs = fixup_result_abs.warnings.join("\n");
    assert!(
        warnings_abs.contains("Path validation failed") || warnings_abs.contains("Absolute") || warnings_abs.contains("absolute"),
        "Warnings should mention absolute path rejection: {}", warnings_abs
    );

    Ok(())
}
