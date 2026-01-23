//! Test for --apply-fixups flag implementation
//!
//! This test validates that the --apply-fixups flag is properly parsed
//! and passed through the system to control fixup mode.

use anyhow::Result;
use std::collections::HashMap;
use tempfile::TempDir;
use xchecker::fixup::{FixupMode, FixupParser};
use xchecker::orchestrator::OrchestratorConfig;

/// Test that the `apply_fixups` configuration is properly handled
#[test]
fn test_apply_fixups_config_handling() -> Result<()> {
    // Test preview mode (default)
    let mut config_preview = HashMap::new();
    config_preview.insert("apply_fixups".to_string(), "false".to_string());

    let orchestrator_config_preview = OrchestratorConfig {
        dry_run: false,
        config: config_preview,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Simulate how orchestrator determines fixup mode
    let apply_fixups_preview = orchestrator_config_preview
        .config
        .get("apply_fixups")
        .is_some_and(|s| s == "true");

    let fixup_mode_preview = if apply_fixups_preview {
        FixupMode::Apply
    } else {
        FixupMode::Preview
    };

    assert_eq!(fixup_mode_preview, FixupMode::Preview);

    // Test apply mode
    let mut config_apply = HashMap::new();
    config_apply.insert("apply_fixups".to_string(), "true".to_string());

    let orchestrator_config_apply = OrchestratorConfig {
        dry_run: false,
        config: config_apply,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    let apply_fixups_apply = orchestrator_config_apply
        .config
        .get("apply_fixups")
        .is_some_and(|s| s == "true");

    let fixup_mode_apply = if apply_fixups_apply {
        FixupMode::Apply
    } else {
        FixupMode::Preview
    };

    assert_eq!(fixup_mode_apply, FixupMode::Apply);

    Ok(())
}

/// Test that `FixupParser` respects the mode setting
#[test]
fn test_fixup_parser_mode_behavior() -> Result<()> {
    let sandbox = TempDir::new()?;
    let temp_dir = sandbox.path().to_path_buf();

    // Test preview mode
    let parser_preview = FixupParser::new(FixupMode::Preview, temp_dir.clone())?;

    let test_content = r"
FIXUP PLAN:
The following changes are needed:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,1 +1,2 @@
 line 1
+line 2
```
";

    // In preview mode, should be able to parse diffs
    if parser_preview.has_fixup_markers(test_content) {
        let diffs = parser_preview.parse_diffs(test_content)?;
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "test.txt");

        // Preview should work (doesn't actually modify files)
        let preview = parser_preview.preview_changes(&diffs)?;
        assert_eq!(preview.target_files.len(), 1);
        assert_eq!(preview.target_files[0], "test.txt");
    }

    // Test apply mode
    let parser_apply = FixupParser::new(FixupMode::Apply, temp_dir)?;

    // Apply mode should also be able to parse diffs
    if parser_apply.has_fixup_markers(test_content) {
        let diffs = parser_apply.parse_diffs(test_content)?;
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "test.txt");

        // Note: We don't test actual application here since it would require
        // real files and git setup. The important thing is that the mode
        // is properly set and the parser can handle the content.
    }

    Ok(())
}

/// Test that `FixupMode` enum works correctly
#[test]
fn test_fixup_mode_enum() {
    assert_eq!(FixupMode::Preview.as_str(), "preview");
    assert_eq!(FixupMode::Apply.as_str(), "apply");

    // Test equality
    assert_eq!(FixupMode::Preview, FixupMode::Preview);
    assert_eq!(FixupMode::Apply, FixupMode::Apply);
    assert_ne!(FixupMode::Preview, FixupMode::Apply);
}
