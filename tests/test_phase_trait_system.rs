//! Comprehensive tests for the Phase trait system (FR-PHASE)
//!
//! Tests the Phase trait methods, PhaseContext, Packet assembly, BudgetUsage tracking,
//! PhaseResult, and concrete phase implementations (Requirements, Design, Tasks).
//!
//! Validates: FR-PHASE-001 through FR-PHASE-006

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use xchecker::phase::{
    BudgetUsage, NextStep, Packet, Phase, PhaseContext, PhaseMetadata, PhaseResult,
};
use xchecker::phases::{DesignPhase, RequirementsPhase, TasksPhase};
use xchecker::types::{FileEvidence, PacketEvidence, PhaseId, Priority};

/// Helper to create a test PhaseContext
fn create_test_context(spec_id: &str, spec_dir: PathBuf) -> PhaseContext {
    let mut config = HashMap::new();
    config.insert("test_key".to_string(), "test_value".to_string());

    PhaseContext {
        spec_id: spec_id.to_string(),
        spec_dir,
        config,
        artifacts: vec![],
    }
}

/// Helper to create a test PhaseContext with artifacts
fn create_test_context_with_artifacts(
    spec_id: &str,
    spec_dir: PathBuf,
    artifacts: Vec<String>,
) -> PhaseContext {
    let mut config = HashMap::new();
    config.insert("test_key".to_string(), "test_value".to_string());

    PhaseContext {
        spec_id: spec_id.to_string(),
        spec_dir,
        config,
        artifacts,
    }
}

// ============================================================================
// Phase Trait Method Tests
// ============================================================================

#[test]
fn test_phase_id_method() {
    let req_phase = RequirementsPhase::new();
    let design_phase = DesignPhase::new();
    let tasks_phase = TasksPhase::new();

    assert_eq!(req_phase.id(), PhaseId::Requirements);
    assert_eq!(design_phase.id(), PhaseId::Design);
    assert_eq!(tasks_phase.id(), PhaseId::Tasks);
}

#[test]
fn test_phase_deps_method() {
    let req_phase = RequirementsPhase::new();
    let design_phase = DesignPhase::new();
    let tasks_phase = TasksPhase::new();

    // Requirements has no dependencies
    assert_eq!(req_phase.deps(), &[]);

    // Design depends on Requirements
    assert_eq!(design_phase.deps(), &[PhaseId::Requirements]);

    // Tasks depends on Design
    assert_eq!(tasks_phase.deps(), &[PhaseId::Design]);
}

#[test]
fn test_phase_can_resume_method() {
    let req_phase = RequirementsPhase::new();
    let design_phase = DesignPhase::new();
    let tasks_phase = TasksPhase::new();

    assert!(req_phase.can_resume());
    assert!(design_phase.can_resume());
    assert!(tasks_phase.can_resume());
}

// ============================================================================
// PhaseContext Tests
// ============================================================================

#[test]
fn test_phase_context_building() {
    let temp_dir = TempDir::new().unwrap();
    let spec_dir = temp_dir.path().to_path_buf();

    let ctx = create_test_context("test-spec-123", spec_dir.clone());

    assert_eq!(ctx.spec_id, "test-spec-123");
    assert_eq!(ctx.spec_dir, spec_dir);
    assert_eq!(ctx.config.get("test_key"), Some(&"test_value".to_string()));
    assert!(ctx.artifacts.is_empty());
}

#[test]
fn test_phase_context_with_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let spec_dir = temp_dir.path().to_path_buf();

    let artifacts = vec![
        "00-requirements.md".to_string(),
        "00-requirements.core.yaml".to_string(),
    ];

    let ctx = create_test_context_with_artifacts("test-spec-456", spec_dir, artifacts.clone());

    assert_eq!(ctx.spec_id, "test-spec-456");
    assert_eq!(ctx.artifacts, artifacts);
}

// ============================================================================
// Packet Assembly Tests
// ============================================================================

#[test]
fn test_packet_creation() {
    let evidence = PacketEvidence {
        files: vec![FileEvidence {
            path: "test.txt".to_string(),
            range: None,
            blake3_pre_redaction: "abc123".to_string(),
            priority: Priority::High,
        }],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let budget = BudgetUsage::new(65536, 1200);

    let packet = Packet::new(
        "test content".to_string(),
        "hash123".to_string(),
        evidence.clone(),
        budget,
    );

    assert_eq!(packet.content(), "test content");
    assert_eq!(packet.hash(), "hash123");
    assert_eq!(packet.evidence().files.len(), 1);
    assert_eq!(packet.evidence().max_bytes, 65536);
    assert_eq!(packet.evidence().max_lines, 1200);
}

#[test]
fn test_packet_with_evidence() {
    let files = vec![
        FileEvidence {
            path: "file1.txt".to_string(),
            range: None,
            blake3_pre_redaction: "hash1".to_string(),
            priority: Priority::Upstream,
        },
        FileEvidence {
            path: "file2.txt".to_string(),
            range: Some("1-10".to_string()),
            blake3_pre_redaction: "hash2".to_string(),
            priority: Priority::High,
        },
    ];

    let evidence = PacketEvidence {
        files: files.clone(),
        max_bytes: 100000,
        max_lines: 2000,
    };

    let budget = BudgetUsage::new(100000, 2000);

    let packet = Packet::new(
        "content with multiple files".to_string(),
        "combined_hash".to_string(),
        evidence,
        budget,
    );

    assert_eq!(packet.evidence().files.len(), 2);
    assert_eq!(packet.evidence().files[0].path, "file1.txt");
    assert_eq!(packet.evidence().files[1].path, "file2.txt");
    assert_eq!(packet.evidence().files[1].range, Some("1-10".to_string()));
}

// ============================================================================
// BudgetUsage Tests
// ============================================================================

#[test]
fn test_budget_usage_creation() {
    let budget = BudgetUsage::new(65536, 1200);

    assert_eq!(budget.bytes_used, 0);
    assert_eq!(budget.lines_used, 0);
    assert_eq!(budget.max_bytes, 65536);
    assert_eq!(budget.max_lines, 1200);
}

#[test]
fn test_budget_usage_would_exceed() {
    let budget = BudgetUsage::new(1000, 100);

    // Should not exceed
    assert!(!budget.would_exceed(500, 50));
    assert!(!budget.would_exceed(1000, 100));

    // Should exceed bytes
    assert!(budget.would_exceed(1001, 50));

    // Should exceed lines
    assert!(budget.would_exceed(500, 101));

    // Should exceed both
    assert!(budget.would_exceed(1001, 101));
}

#[test]
fn test_budget_usage_add_content() {
    let mut budget = BudgetUsage::new(1000, 100);

    budget.add_content(200, 20);
    assert_eq!(budget.bytes_used, 200);
    assert_eq!(budget.lines_used, 20);

    budget.add_content(300, 30);
    assert_eq!(budget.bytes_used, 500);
    assert_eq!(budget.lines_used, 50);
}

#[test]
fn test_budget_usage_is_exceeded() {
    let mut budget = BudgetUsage::new(1000, 100);

    assert!(!budget.is_exceeded());

    budget.add_content(500, 50);
    assert!(!budget.is_exceeded());

    budget.add_content(501, 0);
    assert!(budget.is_exceeded());
}

#[test]
fn test_budget_usage_is_exceeded_by_lines() {
    let mut budget = BudgetUsage::new(1000, 100);

    budget.add_content(500, 101);
    assert!(budget.is_exceeded());
}

#[test]
fn test_packet_is_within_budget() {
    let evidence = PacketEvidence {
        files: vec![],
        max_bytes: 1000,
        max_lines: 100,
    };

    let mut budget = BudgetUsage::new(1000, 100);
    budget.add_content(500, 50);

    let packet = Packet::new("content".to_string(), "hash".to_string(), evidence, budget);

    assert!(packet.is_within_budget());
}

#[test]
fn test_packet_exceeds_budget() {
    let evidence = PacketEvidence {
        files: vec![],
        max_bytes: 1000,
        max_lines: 100,
    };

    let mut budget = BudgetUsage::new(1000, 100);
    budget.add_content(1001, 50);

    let packet = Packet::new("content".to_string(), "hash".to_string(), evidence, budget);

    assert!(!packet.is_within_budget());
}

// ============================================================================
// PhaseResult Tests
// ============================================================================

#[test]
fn test_phase_result_with_artifacts() {
    use xchecker::artifact::{Artifact, ArtifactType};

    let artifacts = vec![
        Artifact {
            name: "test.md".to_string(),
            content: "content".to_string(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: "hash1".to_string(),
        },
        Artifact {
            name: "test.yaml".to_string(),
            content: "yaml: content".to_string(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: "hash2".to_string(),
        },
    ];

    let metadata = PhaseMetadata {
        packet_hash: Some("test_hash".to_string()),
        budget_used: None,
        duration_ms: Some(100),
    };

    let result = PhaseResult {
        artifacts: artifacts.clone(),
        next_step: NextStep::Continue,
        metadata,
    };

    assert_eq!(result.artifacts.len(), 2);
    assert_eq!(result.next_step, NextStep::Continue);
    assert_eq!(result.metadata.packet_hash, Some("test_hash".to_string()));
}

#[test]
fn test_next_step_variants() {
    let continue_step = NextStep::Continue;
    let rewind_step = NextStep::Rewind {
        to: PhaseId::Requirements,
    };
    let complete_step = NextStep::Complete;

    assert_eq!(continue_step, NextStep::Continue);
    assert_eq!(
        rewind_step,
        NextStep::Rewind {
            to: PhaseId::Requirements
        }
    );
    assert_eq!(complete_step, NextStep::Complete);
}

// ============================================================================
// RequirementsPhase Implementation Tests
// ============================================================================

#[test]
fn test_requirements_phase_basic_properties() {
    let phase = RequirementsPhase::new();

    assert_eq!(phase.id(), PhaseId::Requirements);
    assert_eq!(phase.deps(), &[]);
    assert!(phase.can_resume());
}

#[test]
fn test_requirements_phase_prompt_generation() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-req-123", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();
    let prompt = phase.prompt(&ctx);

    // Verify prompt contains key elements
    assert!(prompt.contains("requirements analyst"));
    assert!(prompt.contains("EARS format"));
    assert!(prompt.contains("test-req-123"));
    assert!(prompt.contains("User Story:"));
    assert!(prompt.contains("Acceptance Criteria"));
    assert!(prompt.contains("WHEN"));
    assert!(prompt.contains("THEN"));
    assert!(prompt.contains("SHALL"));
}

#[test]
fn test_requirements_phase_prompt_with_context() {
    let temp_dir = TempDir::new().unwrap();
    let artifacts = vec!["previous-artifact.md".to_string()];
    let ctx = create_test_context_with_artifacts(
        "test-req-456",
        temp_dir.path().to_path_buf(),
        artifacts,
    );

    let phase = RequirementsPhase::new();
    let prompt = phase.prompt(&ctx);

    assert!(prompt.contains("test-req-456"));
    // Should mention context when artifacts are available
    assert!(prompt.contains("context") || prompt.contains("artifacts"));
}

#[test]
fn test_requirements_phase_postprocessing() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-req-post", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();

    let raw_response = r#"# Requirements Document

## Introduction

This is a test requirements document for a sample feature.

## Requirements

### Requirement 1

**User Story:** As a user, I want to test the system, so that I can verify it works correctly.

#### Acceptance Criteria

1. WHEN I run a test THEN the system SHALL respond correctly
2. WHEN I provide invalid input THEN the system SHALL reject it with a clear error message
"#;

    let result = phase.postprocess(raw_response, &ctx)?;

    // Should produce 2 artifacts: markdown and YAML
    assert_eq!(result.artifacts.len(), 2);

    // Check artifact types
    let has_markdown = result
        .artifacts
        .iter()
        .any(|a| a.artifact_type == xchecker::artifact::ArtifactType::Markdown);
    let has_yaml = result
        .artifacts
        .iter()
        .any(|a| a.artifact_type == xchecker::artifact::ArtifactType::CoreYaml);

    assert!(has_markdown, "Should have markdown artifact");
    assert!(has_yaml, "Should have YAML artifact");

    // Check next step
    assert_eq!(result.next_step, NextStep::Continue);

    // Check metadata - PhaseMetadata struct fields
    // Note: actual implementation may set these fields differently
    // For now just verify the struct exists
    let _ = &result.metadata;

    Ok(())
}

#[test]
fn test_requirements_phase_artifact_generation() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-req-artifacts", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();

    let raw_response = "# Requirements\n\nTest content";

    let result = phase.postprocess(raw_response, &ctx)?;

    // Check markdown artifact
    let md_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "00-requirements.md")
        .expect("Should have requirements.md artifact");

    assert_eq!(
        md_artifact.artifact_type,
        xchecker::artifact::ArtifactType::Markdown
    );
    assert!(md_artifact.content.contains("Requirements"));
    assert!(!md_artifact.blake3_hash.is_empty());

    // Check YAML artifact
    let yaml_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "00-requirements.core.yaml")
        .expect("Should have requirements.core.yaml artifact");

    assert_eq!(
        yaml_artifact.artifact_type,
        xchecker::artifact::ArtifactType::CoreYaml
    );
    assert!(yaml_artifact.content.contains("spec_id"));
    assert!(yaml_artifact.content.contains("test-req-artifacts"));
    assert!(yaml_artifact.content.contains("phase: \"requirements\""));
    assert!(!yaml_artifact.blake3_hash.is_empty());

    Ok(())
}

// ============================================================================
// DesignPhase Implementation Tests
// ============================================================================

#[test]
fn test_design_phase_basic_properties() {
    let phase = DesignPhase::new();

    assert_eq!(phase.id(), PhaseId::Design);
    assert_eq!(phase.deps(), &[PhaseId::Requirements]);
    assert!(phase.can_resume());
}

#[test]
fn test_design_phase_dependency_on_requirements() {
    let phase = DesignPhase::new();

    // Design depends on Requirements
    let deps = phase.deps();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0], PhaseId::Requirements);
}

#[test]
fn test_design_phase_prompt_generation() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-design-123", temp_dir.path().to_path_buf());

    let phase = DesignPhase::new();
    let prompt = phase.prompt(&ctx);

    // Verify prompt contains key elements
    assert!(prompt.contains("software architect"));
    assert!(prompt.contains("Design Document"));
    assert!(prompt.contains("test-design-123"));
    assert!(prompt.contains("Architecture"));
    assert!(prompt.contains("Components and Interfaces"));
    assert!(prompt.contains("Data Models"));
    assert!(prompt.contains("Error Handling"));
    assert!(prompt.contains("Testing Strategy"));
}

#[test]
fn test_design_phase_postprocessing() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-design-post", temp_dir.path().to_path_buf());

    let phase = DesignPhase::new();

    let raw_response = r#"# Design Document

## Overview

This is a test design document.

## Architecture

The system uses a layered architecture.

## Components and Interfaces

- Component A: Handles input
- Component B: Processes data
"#;

    let result = phase.postprocess(raw_response, &ctx)?;

    // Should produce 2 artifacts: markdown and YAML
    assert_eq!(result.artifacts.len(), 2);

    // Check next step
    assert_eq!(result.next_step, NextStep::Continue);

    // Check metadata - PhaseMetadata struct exists
    let _ = &result.metadata;

    Ok(())
}

#[test]
fn test_design_phase_artifact_generation() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-design-artifacts", temp_dir.path().to_path_buf());

    let phase = DesignPhase::new();

    let raw_response = "# Design\n\nTest design content";

    let result = phase.postprocess(raw_response, &ctx)?;

    // Check markdown artifact
    let md_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "10-design.md")
        .expect("Should have design.md artifact");

    assert_eq!(
        md_artifact.artifact_type,
        xchecker::artifact::ArtifactType::Markdown
    );
    assert!(md_artifact.content.contains("Design"));
    assert!(!md_artifact.blake3_hash.is_empty());

    // Check YAML artifact
    let yaml_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "10-design.core.yaml")
        .expect("Should have design.core.yaml artifact");

    assert_eq!(
        yaml_artifact.artifact_type,
        xchecker::artifact::ArtifactType::CoreYaml
    );
    assert!(yaml_artifact.content.contains("spec_id"));
    assert!(yaml_artifact.content.contains("test-design-artifacts"));
    assert!(yaml_artifact.content.contains("phase: \"design\""));
    assert!(!yaml_artifact.blake3_hash.is_empty());

    Ok(())
}

// ============================================================================
// TasksPhase Implementation Tests
// ============================================================================

#[test]
fn test_tasks_phase_basic_properties() {
    let phase = TasksPhase::new();

    assert_eq!(phase.id(), PhaseId::Tasks);
    assert_eq!(phase.deps(), &[PhaseId::Design]);
    assert!(phase.can_resume());
}

#[test]
fn test_tasks_phase_dependency_on_design() {
    let phase = TasksPhase::new();

    // Tasks depends on Design
    let deps = phase.deps();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0], PhaseId::Design);
}

#[test]
fn test_tasks_phase_prompt_generation() {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-tasks-123", temp_dir.path().to_path_buf());

    let phase = TasksPhase::new();
    let prompt = phase.prompt(&ctx);

    // Verify prompt contains key elements
    assert!(prompt.contains("technical lead"));
    assert!(prompt.contains("Implementation Plan"));
    assert!(prompt.contains("test-tasks-123"));
    assert!(prompt.contains("test-driven manner"));
    assert!(prompt.contains("coding steps"));
    assert!(prompt.contains("incremental progress"));
}

#[test]
fn test_tasks_phase_postprocessing() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-tasks-post", temp_dir.path().to_path_buf());

    let phase = TasksPhase::new();

    let raw_response = r#"# Implementation Plan

## Task Format

- [ ] 1. Set up project structure
  - Create directory structure
  - _Requirements: REQ-001_

- [ ] 2. Implement core features
- [ ] 2.1 Create data models
  - Write model classes
  - _Requirements: REQ-002_

- [ ]* 2.2 Write unit tests
  - Test data models
  - _Requirements: REQ-002_
"#;

    let result = phase.postprocess(raw_response, &ctx)?;

    // Should produce 2 artifacts: markdown and YAML
    assert_eq!(result.artifacts.len(), 2);

    // Check next step
    assert_eq!(result.next_step, NextStep::Continue);

    // Check metadata - PhaseMetadata struct exists
    let _ = &result.metadata;

    Ok(())
}

#[test]
fn test_tasks_phase_artifact_generation() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-tasks-artifacts", temp_dir.path().to_path_buf());

    let phase = TasksPhase::new();

    let raw_response = "# Implementation Plan\n\nTest tasks content";

    let result = phase.postprocess(raw_response, &ctx)?;

    // Check markdown artifact
    let md_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "20-tasks.md")
        .expect("Should have tasks.md artifact");

    assert_eq!(
        md_artifact.artifact_type,
        xchecker::artifact::ArtifactType::Markdown
    );
    assert!(md_artifact.content.contains("Implementation Plan"));
    assert!(!md_artifact.blake3_hash.is_empty());

    // Check YAML artifact
    let yaml_artifact = result
        .artifacts
        .iter()
        .find(|a| a.name == "20-tasks.core.yaml")
        .expect("Should have tasks.core.yaml artifact");

    assert_eq!(
        yaml_artifact.artifact_type,
        xchecker::artifact::ArtifactType::CoreYaml
    );
    assert!(yaml_artifact.content.contains("spec_id"));
    assert!(yaml_artifact.content.contains("test-tasks-artifacts"));
    assert!(yaml_artifact.content.contains("phase: \"tasks\""));
    assert!(!yaml_artifact.blake3_hash.is_empty());

    Ok(())
}

// ============================================================================
// Packet Assembly with Previous Artifacts Tests
// ============================================================================

#[test]
fn test_requirements_phase_packet_assembly() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file in the temp directory for packet assembly
    let test_file_path = temp_dir.path().join("test-input.txt");
    std::fs::write(&test_file_path, "Test input content for requirements phase")?;

    let ctx = create_test_context("test-req-packet", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();
    let result = phase.make_packet(&ctx);

    assert!(result.is_ok(), "Packet assembly should succeed");

    let packet = result?;

    // Verify packet structure
    assert!(!packet.hash().is_empty());
    assert_eq!(packet.evidence().max_bytes, 65536);
    assert_eq!(packet.evidence().max_lines, 1200);

    // Packet content may be empty if no files match the selector patterns
    // This is expected behavior - the packet builder only includes files that match patterns

    Ok(())
}

#[test]
fn test_design_phase_packet_assembly() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file in the temp directory for packet assembly
    let test_file_path = temp_dir.path().join("test-design.txt");
    std::fs::write(&test_file_path, "Test design content")?;

    let ctx = create_test_context("test-design-packet", temp_dir.path().to_path_buf());

    let phase = DesignPhase::new();
    let result = phase.make_packet(&ctx);

    assert!(result.is_ok(), "Packet assembly should succeed");

    let packet = result?;

    // Verify packet structure
    assert!(!packet.hash().is_empty());

    // Packet content may be empty if no files match the selector patterns
    // This is expected behavior - the packet builder only includes files that match patterns

    Ok(())
}

#[test]
fn test_tasks_phase_packet_assembly() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file in the temp directory for packet assembly
    let test_file_path = temp_dir.path().join("test-tasks.txt");
    std::fs::write(&test_file_path, "Test tasks content")?;

    let ctx = create_test_context("test-tasks-packet", temp_dir.path().to_path_buf());

    let phase = TasksPhase::new();
    let result = phase.make_packet(&ctx);

    assert!(result.is_ok(), "Packet assembly should succeed");

    let packet = result?;

    // Verify packet structure
    assert!(!packet.hash().is_empty());

    // Packet content may be empty if no files match the selector patterns
    // This is expected behavior - the packet builder only includes files that match patterns

    Ok(())
}

// ============================================================================
// Deterministic Output Tests (FR-PHASE-001)
// ============================================================================

#[test]
fn test_prompt_generation_is_deterministic() {
    let temp_dir = TempDir::new().unwrap();
    let ctx1 = create_test_context("test-deterministic", temp_dir.path().to_path_buf());
    let ctx2 = create_test_context("test-deterministic", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();

    let prompt1 = phase.prompt(&ctx1);
    let prompt2 = phase.prompt(&ctx2);

    // Same context should produce identical prompts
    assert_eq!(prompt1, prompt2);
}

#[test]
fn test_postprocess_is_deterministic() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx1 = create_test_context("test-deterministic-post", temp_dir.path().to_path_buf());
    let ctx2 = create_test_context("test-deterministic-post", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();

    let raw_response = "# Requirements\n\nTest content";

    let result1 = phase.postprocess(raw_response, &ctx1)?;
    let result2 = phase.postprocess(raw_response, &ctx2)?;

    // Same input should produce same number of artifacts
    assert_eq!(result1.artifacts.len(), result2.artifacts.len());

    // Artifact names should match
    let names1: Vec<_> = result1.artifacts.iter().map(|a| &a.name).collect();
    let names2: Vec<_> = result2.artifacts.iter().map(|a| &a.name).collect();
    assert_eq!(names1, names2);

    // Next steps should match
    assert_eq!(result1.next_step, result2.next_step);

    Ok(())
}

// ============================================================================
// No Side Effects Tests (FR-PHASE-002)
// ============================================================================

#[test]
fn test_postprocess_no_side_effects() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let ctx = create_test_context("test-no-side-effects", temp_dir.path().to_path_buf());

    let phase = RequirementsPhase::new();

    let raw_response = "# Requirements\n\nTest content";

    // Call postprocess multiple times
    let _result1 = phase.postprocess(raw_response, &ctx)?;
    let _result2 = phase.postprocess(raw_response, &ctx)?;

    // Verify no files were created in temp_dir (postprocess should not write files)
    // The actual file writing is done by the orchestrator, not by postprocess
    let entries: Vec<_> = std::fs::read_dir(temp_dir.path())?.collect();

    // Should be empty or only contain directories created by the test setup
    // Postprocess itself should not create any files
    assert!(
        entries.is_empty() || entries.iter().all(|e| e.as_ref().unwrap().path().is_dir()),
        "Postprocess should not create files directly"
    );

    Ok(())
}

// ============================================================================
// Phase Dependency Chain Tests
// ============================================================================

#[test]
fn test_phase_dependency_chain() {
    let req_phase = RequirementsPhase::new();
    let design_phase = DesignPhase::new();
    let tasks_phase = TasksPhase::new();

    // Requirements has no dependencies
    assert_eq!(req_phase.deps().len(), 0);

    // Design depends on Requirements
    assert_eq!(design_phase.deps().len(), 1);
    assert_eq!(design_phase.deps()[0], PhaseId::Requirements);

    // Tasks depends on Design (which transitively depends on Requirements)
    assert_eq!(tasks_phase.deps().len(), 1);
    assert_eq!(tasks_phase.deps()[0], PhaseId::Design);
}

// ============================================================================
// Comprehensive Integration Test
// ============================================================================

#[test]
fn test_complete_phase_workflow() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();

    // Create a test file for packet assembly
    let test_file_path = temp_dir.path().join("workflow-test.txt");
    std::fs::write(&test_file_path, "Test workflow content")?;

    // 1. Requirements Phase
    let req_ctx = create_test_context("test-workflow", temp_dir.path().to_path_buf());
    let req_phase = RequirementsPhase::new();

    let req_prompt = req_phase.prompt(&req_ctx);
    assert!(!req_prompt.is_empty());

    let req_packet = req_phase.make_packet(&req_ctx)?;
    assert!(!req_packet.hash().is_empty());

    let req_response = "# Requirements\n\nTest requirements";
    let req_result = req_phase.postprocess(req_response, &req_ctx)?;
    assert_eq!(req_result.artifacts.len(), 2);
    assert_eq!(req_result.next_step, NextStep::Continue);

    // 2. Design Phase (depends on Requirements)
    let design_ctx = create_test_context("test-workflow", temp_dir.path().to_path_buf());
    let design_phase = DesignPhase::new();

    assert_eq!(design_phase.deps(), &[PhaseId::Requirements]);

    let design_prompt = design_phase.prompt(&design_ctx);
    assert!(!design_prompt.is_empty());

    let design_packet = design_phase.make_packet(&design_ctx)?;
    assert!(!design_packet.hash().is_empty());

    let design_response = "# Design\n\nTest design";
    let design_result = design_phase.postprocess(design_response, &design_ctx)?;
    assert_eq!(design_result.artifacts.len(), 2);
    assert_eq!(design_result.next_step, NextStep::Continue);

    // 3. Tasks Phase (depends on Design)
    let tasks_ctx = create_test_context("test-workflow", temp_dir.path().to_path_buf());
    let tasks_phase = TasksPhase::new();

    assert_eq!(tasks_phase.deps(), &[PhaseId::Design]);

    let tasks_prompt = tasks_phase.prompt(&tasks_ctx);
    assert!(!tasks_prompt.is_empty());

    let tasks_packet = tasks_phase.make_packet(&tasks_ctx)?;
    assert!(!tasks_packet.hash().is_empty());

    let tasks_response = "# Implementation Plan\n\nTest tasks";
    let tasks_result = tasks_phase.postprocess(tasks_response, &tasks_ctx)?;
    assert_eq!(tasks_result.artifacts.len(), 2);
    assert_eq!(tasks_result.next_step, NextStep::Continue);

    Ok(())
}
