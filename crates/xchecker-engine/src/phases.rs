//! Concrete implementations of workflow phases
//!
//! This module contains the specific implementations of each phase in the
//! spec generation workflow, starting with the Requirements phase.

use anyhow::Result;
use camino::Utf8PathBuf;
use std::collections::HashMap;

use crate::artifact::{Artifact, ArtifactType};
use crate::cache::InsightCache;
use crate::extraction::{summarize_design, summarize_requirements, summarize_tasks};
use crate::fixup::{FixupMode, FixupParser};
use crate::packet::{DEFAULT_PACKET_MAX_BYTES, DEFAULT_PACKET_MAX_LINES, PacketBuilder};
use crate::paths;
use crate::phase::{
    BudgetUsage, NextStep, Packet, Phase, PhaseContext, PhaseMetadata, PhaseResult,
};
use crate::types::PacketEvidence;
use crate::types::PhaseId;
use crate::validation::OutputValidator;

/// Common anti-summary instructions appended to all generative phase prompts.
/// This prevents the LLM from outputting meta-commentary instead of actual content.
const ANTI_SUMMARY_INSTRUCTIONS: &str = "

CRITICAL OUTPUT RULES - YOU MUST FOLLOW THESE:
1. Output the ACTUAL document content directly - no meta-commentary
2. Do NOT start with phrases like 'I will create...', 'Here is...', 'I have created...', 'Perfect!', 'Great!', etc.
3. Start IMMEDIATELY with the document header (e.g., '# Requirements Document')
4. Do NOT summarize or describe what the document contains - BE the document
5. Do NOT include phrases like 'based on the context' or 'as requested'
6. Your entire response should be the document itself, nothing else

WRONG (will be rejected):
  'I have created a comprehensive requirements document with 5 user stories...'
  'Here is the design document you requested...'
  'Perfect! Based on the requirements, I will create...'

CORRECT (start immediately with content):
  # Requirements Document

  ## Introduction

  This system provides...";

fn packet_limits_from_context(ctx: &PhaseContext) -> (usize, usize) {
    let max_bytes = ctx
        .config
        .get("packet_max_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PACKET_MAX_BYTES);
    let max_lines = ctx
        .config
        .get("packet_max_lines")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PACKET_MAX_LINES);
    (max_bytes, max_lines)
}

fn build_packet_builder(ctx: &PhaseContext) -> Result<PacketBuilder> {
    let (max_bytes, max_lines) = packet_limits_from_context(ctx);
    let mut builder =
        PacketBuilder::with_selectors_and_limits(ctx.selectors.as_ref(), max_bytes, max_lines)?;
    *builder.redactor_mut() = (*ctx.redactor).clone();

    match InsightCache::new(paths::cache_dir()) {
        Ok(cache) => builder.set_cache(cache),
        Err(err) => {
            tracing::warn!(
                "Failed to initialize InsightCache (continuing without cache): {}",
                err
            );
        }
    }

    Ok(builder)
}

/// Implementation of the Requirements phase
///
/// This phase takes a rough problem statement and generates structured requirements
/// in EARS format (Easy Approach to Requirements Syntax) with user stories and
/// acceptance criteria.
#[derive(Debug, Clone)]
pub struct RequirementsPhase;

impl RequirementsPhase {
    /// Create a new Requirements phase instance
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Phase for RequirementsPhase {
    fn id(&self) -> PhaseId {
        PhaseId::Requirements
    }

    fn deps(&self) -> &'static [PhaseId] {
        // Requirements phase has no dependencies - it's the first phase
        &[]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, ctx: &PhaseContext) -> String {
        // Generate the requirements phase prompt with problem statement from config
        let problem_statement = ctx
            .config
            .get("problem_statement")
            .map(String::as_str)
            .unwrap_or("No explicit problem statement was provided. Please analyze the context packet for requirements.");

        format!(
            r"You are a requirements analyst helping to transform a rough feature idea into structured requirements.

Spec ID: {}

# Problem Statement

{}

# Your Task

Create a comprehensive requirements document that follows this format:

# Requirements Document

## Introduction

[Provide a clear introduction that summarizes the feature and its purpose]

## Requirements

### Requirement 1

**User Story:** As a [role], I want [feature], so that [benefit]

#### Acceptance Criteria

1. WHEN [event] THEN [system] SHALL [response]
2. IF [precondition] THEN [system] SHALL [response]
3. WHEN [event] AND [condition] THEN [system] SHALL [response]

### Requirement 2

**User Story:** As a [role], I want [feature], so that [benefit]

#### Acceptance Criteria

1. WHEN [event] THEN [system] SHALL [response]
2. WHEN [event] THEN [system] SHALL [response]

[Continue with additional requirements as needed]

## Non-Functional Requirements

**NFR1 [Category]:** [Specific measurable requirement]
**NFR2 [Category]:** [Specific measurable requirement]

Guidelines:
- Use EARS format (Easy Approach to Requirements Syntax) for acceptance criteria
- Each requirement should have a clear user story and specific acceptance criteria
- Consider edge cases, error conditions, and user experience
- Include non-functional requirements for performance, security, usability, etc.
- Be specific and testable - avoid vague language
- Focus on WHAT the system should do, not HOW it should do it

Please analyze the problem statement above and create structured requirements following the format.{}",
            ctx.spec_id,
            problem_statement.trim(),
            ANTI_SUMMARY_INSTRUCTIONS,
        )
    }

    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet> {
        // Use PacketBuilder to construct packet with proper budget enforcement
        // and secret scanning (FR-PKT, FR-SEC)

        // Convert spec_dir to Utf8PathBuf for PacketBuilder
        let base_path = Utf8PathBuf::try_from(ctx.spec_dir.clone())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {e}"))?;

        // Create context directory path
        let context_dir = base_path.join("context");

        // Create PacketBuilder with selectors from context (if configured)
        let mut builder = build_packet_builder(ctx)?;

        // Build packet from base path
        // PacketBuilder will:
        // - Select files based on priority (Upstream > High > Medium > Low)
        // - Scan for secrets before including content
        // - Enforce budget limits (exit 7 if exceeded)
        // - Write packet preview to context/requirements-packet.txt
        // - Track file evidence with blake3_pre_redaction hashes
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        Ok(packet)
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        // Process Claude's response into requirements artifacts
        let requirements_content = raw.trim().to_string();

        // Validate the response content
        if let Err(errors) = OutputValidator::validate(&requirements_content, PhaseId::Requirements)
        {
            // Always log validation issues (with redaction for FR-SEC-19)
            for err in &errors {
                let redacted_err = ctx.redactor.redact_string(&err.to_string());
                eprintln!(
                    "[WARN] Validation issue in requirements output: {}",
                    redacted_err
                );
            }

            // In strict mode, fail the phase
            if ctx.strict_validation {
                return Err(crate::error::XCheckerError::ValidationFailed {
                    phase: "requirements".to_string(),
                    issue_count: errors.len(),
                    issues: errors,
                }
                .into());
            }
        }

        // Create the main requirements.md artifact
        let requirements_artifact = Artifact {
            name: "00-requirements.md".to_string(),
            content: requirements_content.clone(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: blake3::hash(requirements_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        // Create a core YAML artifact with structured data
        // This would contain the parsed requirements in a structured format
        let core_yaml_content = self.generate_core_yaml(&requirements_content, ctx)?;
        let core_yaml_artifact = Artifact {
            name: "00-requirements.core.yaml".to_string(),
            content: core_yaml_content.clone(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: blake3::hash(core_yaml_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let artifacts = vec![requirements_artifact, core_yaml_artifact];

        // Metadata will be populated by orchestrator with packet hash, budget, and duration
        let metadata = PhaseMetadata::default();

        Ok(PhaseResult {
            artifacts,
            next_step: NextStep::Continue, // Proceed to Design phase
            metadata,
        })
    }
}

impl RequirementsPhase {
    /// Generate a core YAML file with structured requirements data
    ///
    /// This creates a machine-readable representation of the requirements
    /// that can be used by subsequent phases. Uses B3.0 minimal extraction
    /// to populate metadata counts from the markdown content.
    fn generate_core_yaml(&self, requirements_md: &str, ctx: &PhaseContext) -> Result<String> {
        // B3.0: Extract summary metadata from the markdown
        let summary = summarize_requirements(requirements_md);

        let yaml_content = format!(
            r#"# Core requirements data for spec {}
# This file contains structured data extracted from the requirements document

spec_id: "{}"
phase: "requirements"
version: "1.0"

# Metadata about the requirements (B3.0 extraction)
metadata:
  total_requirements: {}
  total_user_stories: {}
  total_acceptance_criteria: {}
  total_nfrs: {}
  has_nfrs: {}

# Structured requirements data (B3.1 - future)
requirements: []

# Non-functional requirements (B3.1 - future)
nfrs: []

# Dependencies and relationships
dependencies: []

# Generated timestamp
generated_at: "{}"
"#,
            ctx.spec_id,
            ctx.spec_id,
            summary.requirement_count,
            summary.user_story_count,
            summary.acceptance_criteria_count,
            summary.nfr_count,
            summary.nfr_count > 0,
            chrono::Utc::now().to_rfc3339()
        );

        Ok(yaml_content)
    }
}

impl Default for RequirementsPhase {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation of the Design phase
///
/// This phase takes the requirements document and generates a comprehensive
/// design document with architecture, components, interfaces, and data models.
#[derive(Debug, Clone)]
pub struct DesignPhase;

impl DesignPhase {
    /// Create a new Design phase instance
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Phase for DesignPhase {
    fn id(&self) -> PhaseId {
        PhaseId::Design
    }

    fn deps(&self) -> &'static [PhaseId] {
        // Design phase depends on Requirements phase
        &[PhaseId::Requirements]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, ctx: &PhaseContext) -> String {
        format!(
            r"You are a software architect helping to transform structured requirements into a comprehensive design document.

Your task is to create a detailed design document that follows this format:

# Design Document

## Overview

[Provide a clear overview of the system and its core design principles]

## Architecture

[Describe the high-level architecture with diagrams if appropriate using Mermaid]

## Components and Interfaces

[Detail the major components and their interfaces]

## Data Models

[Define the data structures and their relationships]

## Error Handling

[Describe error handling strategies and patterns]

## Testing Strategy

[Outline the testing approach and strategies]

Guidelines:
- Base the design on the requirements document provided in the context
- Include architectural diagrams using Mermaid syntax where helpful
- Focus on component interfaces and data flow
- Address all functional and non-functional requirements
- Consider scalability, maintainability, and security
- Be specific about technology choices and design patterns
- Include error handling and edge case considerations

Spec ID: {}
Phase: Design

Please analyze the requirements and create a comprehensive design document following the format above.{}",
            ctx.spec_id, ANTI_SUMMARY_INSTRUCTIONS,
        )
    }

    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet> {
        // Use PacketBuilder to construct packet with proper budget enforcement
        // and secret scanning (FR-PKT, FR-SEC)

        // Convert spec_dir to Utf8PathBuf for PacketBuilder
        let base_path = Utf8PathBuf::try_from(ctx.spec_dir.clone())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {e}"))?;

        // Create context directory path
        let context_dir = base_path.join("context");

        // Create PacketBuilder with selectors from context (if configured)
        let mut builder = build_packet_builder(ctx)?;

        // Build packet from base path
        // PacketBuilder will:
        // - Select files based on priority (Upstream > High > Medium > Low)
        // - Include artifacts from previous phases (requirements.md, requirements.core.yaml)
        // - Scan for secrets before including content
        // - Enforce budget limits (exit 7 if exceeded)
        // - Write packet preview to context/design-packet.txt
        // - Track file evidence with blake3_pre_redaction hashes
        let packet = builder.build_packet(&base_path, "design", &context_dir, None)?;

        Ok(packet)
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        // Process Claude's response into design artifacts
        let design_content = raw.trim().to_string();

        // Validate the response content
        if let Err(errors) = OutputValidator::validate(&design_content, PhaseId::Design) {
            // Always log validation issues (with redaction for FR-SEC-19)
            for err in &errors {
                let redacted_err = ctx.redactor.redact_string(&err.to_string());
                eprintln!("[WARN] Validation issue in design output: {}", redacted_err);
            }

            // In strict mode, fail the phase
            if ctx.strict_validation {
                return Err(crate::error::XCheckerError::ValidationFailed {
                    phase: "design".to_string(),
                    issue_count: errors.len(),
                    issues: errors,
                }
                .into());
            }
        }

        // Create the main design.md artifact
        let design_artifact = Artifact {
            name: "10-design.md".to_string(),
            content: design_content.clone(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: blake3::hash(design_content.as_bytes()).to_hex().to_string(),
        };

        // Create a core YAML artifact with structured design data
        let core_yaml_content = self.generate_core_yaml(&design_content, ctx)?;
        let core_yaml_artifact = Artifact {
            name: "10-design.core.yaml".to_string(),
            content: core_yaml_content.clone(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: blake3::hash(core_yaml_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let artifacts = vec![design_artifact, core_yaml_artifact];

        // Metadata will be populated by orchestrator with packet hash, budget, and duration
        let metadata = PhaseMetadata::default();

        Ok(PhaseResult {
            artifacts,
            next_step: NextStep::Continue, // Proceed to Tasks phase
            metadata,
        })
    }
}

impl DesignPhase {
    /// Generate a core YAML file with structured design data
    ///
    /// Uses B3.0 minimal extraction to populate metadata from the markdown content.
    fn generate_core_yaml(&self, design_md: &str, ctx: &PhaseContext) -> Result<String> {
        // B3.0: Extract summary metadata from the markdown
        let summary = summarize_design(design_md);

        let yaml_content = format!(
            r#"# Core design data for spec {}
# This file contains structured data extracted from the design document

spec_id: "{}"
phase: "design"
version: "1.0"

# Metadata about the design (B3.0 extraction)
metadata:
  has_architecture_section: {}
  has_mermaid_diagrams: {}
  total_components: {}
  total_interfaces: {}
  total_data_models: {}

# Structured design data (B3.1 - future)
architecture:
  components: []
  interfaces: []
  data_flow: []

# Data models (B3.1 - future)
data_models: []

# Error handling strategies (B3.1 - future)
error_handling: []

# Testing strategies (B3.1 - future)
testing_strategy: []

# Dependencies on requirements
requirements_dependencies: []

# Generated timestamp
generated_at: "{}"
"#,
            ctx.spec_id,
            ctx.spec_id,
            summary.has_architecture,
            summary.has_diagrams,
            summary.component_count,
            summary.interface_count,
            summary.data_model_count,
            chrono::Utc::now().to_rfc3339()
        );

        Ok(yaml_content)
    }
}

impl Default for DesignPhase {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation of the Tasks phase
///
/// This phase takes the design document and generates an actionable implementation
/// plan with a checklist of coding tasks based on the requirements and design.
#[derive(Debug, Clone)]
pub struct TasksPhase;

impl TasksPhase {
    /// Create a new Tasks phase instance
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Phase for TasksPhase {
    fn id(&self) -> PhaseId {
        PhaseId::Tasks
    }

    fn deps(&self) -> &'static [PhaseId] {
        // Tasks phase depends on Design phase (which depends on Requirements)
        &[PhaseId::Design]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, ctx: &PhaseContext) -> String {
        format!(
            r#"You are a technical lead helping to transform a feature design into a series of actionable implementation tasks.

Your task is to create an implementation plan that follows this format:

# Implementation Plan

Convert the feature design into a series of prompts for a code-generation LLM that will implement each step in a test-driven manner. Prioritize best practices, incremental progress, and early testing, ensuring no big jumps in complexity at any stage. Make sure that each prompt builds on the previous prompts, and ends with wiring things together. There should be no hanging or orphaned code that isn't integrated into a previous step. Focus ONLY on tasks that involve writing, modifying, or testing code.

## Task Format

- [ ] 1. Set up project structure and core interfaces
  - Create directory structure for models, services, repositories, and API components
  - Define interfaces that establish system boundaries
  - _Requirements: [Reference specific requirements from requirements document]_

- [ ] 2. Implement data models and validation
- [ ] 2.1 Create core data model interfaces and types
  - Write TypeScript interfaces for all data models
  - Implement validation functions for data integrity
  - _Requirements: [Reference specific requirements]_

- [ ] 2.2 Implement User model with validation
  - Write User class with validation methods
  - _Requirements: [Reference specific requirements]_

- [ ]* 2.3 Write unit tests for data models
  - Create unit tests for User model validation
  - Write unit tests for relationship management
  - _Requirements: [Reference specific requirements]_

Guidelines:
- Convert the design into discrete, manageable coding steps
- Each task must involve writing, modifying, or testing code
- Reference specific requirements from the requirements document
- Build incrementally - each step should build on previous steps
- Mark testing tasks as optional with "*" suffix (e.g., "- [ ]* 2.3 Write unit tests")
- Use maximum two levels of hierarchy (main tasks and sub-tasks)
- Sub-tasks use decimal notation (1.1, 1.2, 2.1, etc.)
- Focus on test-driven development where appropriate
- Ensure all requirements are covered by implementation tasks
- Do NOT include deployment, user testing, or non-coding activities
- Each task should be actionable by a coding agent

Spec ID: {}
Phase: Tasks

Please analyze the design and requirements to create a comprehensive implementation plan following the format above.{}"#,
            ctx.spec_id, ANTI_SUMMARY_INSTRUCTIONS,
        )
    }

    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet> {
        // Use PacketBuilder to construct packet with proper budget enforcement
        // and secret scanning (FR-PKT, FR-SEC)

        // Convert spec_dir to Utf8PathBuf for PacketBuilder
        let base_path = Utf8PathBuf::try_from(ctx.spec_dir.clone())
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {e}"))?;

        // Create context directory path
        let context_dir = base_path.join("context");

        // Create PacketBuilder with selectors from context (if configured)
        let mut builder = build_packet_builder(ctx)?;

        // Build packet from base path
        // PacketBuilder will:
        // - Select files based on priority (Upstream > High > Medium > Low)
        // - Include artifacts from previous phases (requirements, design)
        // - Scan for secrets before including content
        // - Enforce budget limits (exit 7 if exceeded)
        // - Write packet preview to context/tasks-packet.txt
        // - Track file evidence with blake3_pre_redaction hashes
        let packet = builder.build_packet(&base_path, "tasks", &context_dir, None)?;

        Ok(packet)
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        // Process Claude's response into tasks artifacts
        let tasks_content = raw.trim().to_string();

        // Validate the response content
        if let Err(errors) = OutputValidator::validate(&tasks_content, PhaseId::Tasks) {
            // Always log validation issues (with redaction for FR-SEC-19)
            for err in &errors {
                let redacted_err = ctx.redactor.redact_string(&err.to_string());
                eprintln!("[WARN] Validation issue in tasks output: {}", redacted_err);
            }

            // In strict mode, fail the phase
            if ctx.strict_validation {
                return Err(crate::error::XCheckerError::ValidationFailed {
                    phase: "tasks".to_string(),
                    issue_count: errors.len(),
                    issues: errors,
                }
                .into());
            }
        }

        // Create the main tasks.md artifact
        let tasks_artifact = Artifact {
            name: "20-tasks.md".to_string(),
            content: tasks_content.clone(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: blake3::hash(tasks_content.as_bytes()).to_hex().to_string(),
        };

        // Create a core YAML artifact with structured tasks data
        let core_yaml_content = self.generate_core_yaml(&tasks_content, ctx)?;
        let core_yaml_artifact = Artifact {
            name: "20-tasks.core.yaml".to_string(),
            content: core_yaml_content.clone(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: blake3::hash(core_yaml_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let artifacts = vec![tasks_artifact, core_yaml_artifact];

        // Metadata will be populated by orchestrator with packet hash, budget, and duration
        let metadata = PhaseMetadata::default();

        Ok(PhaseResult {
            artifacts,
            next_step: NextStep::Continue, // Proceed to Review phase
            metadata,
        })
    }
}

impl TasksPhase {
    /// Generate a core YAML file with structured tasks data
    ///
    /// Uses B3.0 minimal extraction to populate metadata from the markdown content.
    fn generate_core_yaml(&self, tasks_md: &str, ctx: &PhaseContext) -> Result<String> {
        // B3.0: Extract summary metadata from the markdown
        let summary = summarize_tasks(tasks_md);

        let yaml_content = format!(
            r#"# Core tasks data for spec {}
# This file contains structured data extracted from the tasks document

spec_id: "{}"
phase: "tasks"
version: "1.0"

# Metadata about the tasks (B3.0 extraction)
metadata:
  total_tasks: {}
  total_subtasks: {}
  total_milestones: {}
  total_dependencies: {}

# Structured tasks data (B3.1 - future)
tasks: []

# Task dependencies and ordering (B3.1 - future)
dependencies: []

# Requirements coverage (B3.1 - future)
requirements_coverage: []

# Generated timestamp
generated_at: "{}"
"#,
            ctx.spec_id,
            ctx.spec_id,
            summary.task_count,
            summary.subtask_count,
            summary.milestone_count,
            summary.dependency_count,
            chrono::Utc::now().to_rfc3339()
        );

        Ok(yaml_content)
    }
}

impl Default for TasksPhase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_context() -> PhaseContext {
        PhaseContext {
            spec_id: "test-123".to_string(),
            spec_dir: PathBuf::from("/tmp/test-spec"),
            config: HashMap::new(),
            artifacts: Vec::new(),
            selectors: None,
            strict_validation: false,
            redactor: std::sync::Arc::new(crate::redaction::SecretRedactor::default()),
        }
    }

    #[test]
    fn test_requirements_phase_basic_properties() {
        let phase = RequirementsPhase::new();

        assert_eq!(phase.id(), PhaseId::Requirements);
        assert_eq!(phase.deps(), &[]);
        assert!(phase.can_resume());
    }

    #[test]
    fn test_requirements_phase_prompt_generation() {
        let phase = RequirementsPhase::new();
        let ctx = create_test_context();

        let prompt = phase.prompt(&ctx);

        assert!(prompt.contains("requirements analyst"));
        assert!(prompt.contains("EARS format"));
        assert!(prompt.contains("test-123"));
        assert!(prompt.contains("User Story:"));
        assert!(prompt.contains("Acceptance Criteria"));
    }

    #[test]
    fn test_requirements_phase_packet_creation() {
        let phase = RequirementsPhase::new();
        let ctx = create_test_context();

        let result = phase.make_packet(&ctx);
        assert!(result.is_ok());

        let packet = result.unwrap();
        // Packet content may be empty if no files match the selector patterns
        // This is expected behavior - the packet builder only includes files that match patterns
        assert!(!packet.blake3_hash.is_empty());
        assert_eq!(packet.evidence.max_bytes, 65536);
        assert_eq!(packet.evidence.max_lines, 1200);
    }

    #[test]
    fn test_requirements_phase_postprocessing() {
        let phase = RequirementsPhase::new();
        let ctx = create_test_context();

        let raw_response = r"# Requirements Document

## Introduction

This is a test requirements document.

## Requirements

### Requirement 1

**User Story:** As a user, I want to test the system, so that I can verify it works.

#### Acceptance Criteria

1. WHEN I run a test THEN the system SHALL respond correctly
";

        let result = phase.postprocess(raw_response, &ctx);
        assert!(result.is_ok());

        let phase_result = result.unwrap();
        assert_eq!(phase_result.artifacts.len(), 2);
        assert_eq!(phase_result.next_step, NextStep::Continue);

        // Check that we have both markdown and YAML artifacts
        let artifact_types: Vec<_> = phase_result
            .artifacts
            .iter()
            .map(|a| a.artifact_type)
            .collect();
        assert!(artifact_types.contains(&ArtifactType::Markdown));
        assert!(artifact_types.contains(&ArtifactType::CoreYaml));
    }

    #[test]
    fn test_core_yaml_generation() {
        let phase = RequirementsPhase::new();
        let ctx = create_test_context();

        let requirements_md = "# Test Requirements\n\nSome requirements content";
        let result = phase.generate_core_yaml(requirements_md, &ctx);

        assert!(result.is_ok());
        let yaml_content = result.unwrap();
        assert!(yaml_content.contains("spec_id: \"test-123\""));
        assert!(yaml_content.contains("phase: \"requirements\""));
        assert!(yaml_content.contains("version: \"1.0\""));
    }

    #[test]
    fn test_design_phase_basic_properties() {
        let phase = DesignPhase::new();

        assert_eq!(phase.id(), PhaseId::Design);
        assert_eq!(phase.deps(), &[PhaseId::Requirements]);
        assert!(phase.can_resume());
    }

    #[test]
    fn test_design_phase_prompt_generation() {
        let phase = DesignPhase::new();
        let ctx = create_test_context();

        let prompt = phase.prompt(&ctx);

        assert!(prompt.contains("software architect"));
        assert!(prompt.contains("Design Document"));
        assert!(prompt.contains("test-123"));
        assert!(prompt.contains("Architecture"));
        assert!(prompt.contains("Components and Interfaces"));
    }

    #[test]
    fn test_tasks_phase_basic_properties() {
        let phase = TasksPhase::new();

        assert_eq!(phase.id(), PhaseId::Tasks);
        assert_eq!(phase.deps(), &[PhaseId::Design]);
        assert!(phase.can_resume());
    }

    #[test]
    fn test_tasks_phase_prompt_generation() {
        let phase = TasksPhase::new();
        let ctx = create_test_context();

        let prompt = phase.prompt(&ctx);

        assert!(prompt.contains("technical lead"));
        assert!(prompt.contains("Implementation Plan"));
        assert!(prompt.contains("test-123"));
        assert!(prompt.contains("test-driven manner"));
        assert!(prompt.contains("coding steps"));
    }

    // ===== Strict Validation Phase Behavior Tests (P1 feature) =====

    fn create_strict_context() -> PhaseContext {
        PhaseContext {
            spec_id: "test-strict".to_string(),
            spec_dir: PathBuf::from("/tmp/test-spec"),
            config: HashMap::new(),
            artifacts: Vec::new(),
            selectors: None,
            strict_validation: true,
            redactor: std::sync::Arc::new(crate::redaction::SecretRedactor::default()),
        }
    }

    #[test]
    fn test_requirements_postprocess_soft_mode_allows_invalid_output() {
        let phase = RequirementsPhase::new();
        let ctx = create_test_context(); // strict_validation: false

        // Invalid output with meta-summary
        let raw_response = "I've created a requirements document for you.";

        let result = phase.postprocess(raw_response, &ctx);
        // In soft mode (default), should succeed despite validation issues
        assert!(result.is_ok());
    }

    #[test]
    fn test_requirements_postprocess_strict_mode_fails_on_meta_summary() {
        let phase = RequirementsPhase::new();
        let ctx = create_strict_context(); // strict_validation: true

        // Invalid output with meta-summary
        let raw_response = "I've created a requirements document for you.";

        let result = phase.postprocess(raw_response, &ctx);
        // In strict mode, should fail due to validation issues
        assert!(result.is_err());
    }

    #[test]
    fn test_design_postprocess_soft_mode_allows_invalid_output() {
        let phase = DesignPhase::new();
        let ctx = create_test_context(); // strict_validation: false

        // Invalid output with meta-summary
        let raw_response = "Here is the design document I generated.";

        let result = phase.postprocess(raw_response, &ctx);
        // In soft mode (default), should succeed despite validation issues
        assert!(result.is_ok());
    }

    #[test]
    fn test_design_postprocess_strict_mode_fails_on_meta_summary() {
        let phase = DesignPhase::new();
        let ctx = create_strict_context(); // strict_validation: true

        // Invalid output with meta-summary
        let raw_response = "Here is the design document I generated.";

        let result = phase.postprocess(raw_response, &ctx);
        // In strict mode, should fail due to validation issues
        assert!(result.is_err());
    }

    #[test]
    fn test_tasks_postprocess_soft_mode_allows_invalid_output() {
        let phase = TasksPhase::new();
        let ctx = create_test_context(); // strict_validation: false

        // Invalid output with meta-summary
        let raw_response = "I've generated the implementation tasks.";

        let result = phase.postprocess(raw_response, &ctx);
        // In soft mode (default), should succeed despite validation issues
        assert!(result.is_ok());
    }

    #[test]
    fn test_tasks_postprocess_strict_mode_fails_on_meta_summary() {
        let phase = TasksPhase::new();
        let ctx = create_strict_context(); // strict_validation: true

        // Invalid output with meta-summary
        let raw_response = "I've generated the implementation tasks.";

        let result = phase.postprocess(raw_response, &ctx);
        // In strict mode, should fail due to validation issues
        assert!(result.is_err());
    }

    #[test]
    fn test_requirements_postprocess_strict_mode_passes_valid_output() {
        let phase = RequirementsPhase::new();
        let ctx = create_strict_context(); // strict_validation: true

        // Valid requirements document (not a meta-summary, meets minimum length)
        let raw_response = r"# Requirements Document

## Introduction

This requirements document specifies the functional and non-functional requirements for the system.
The document follows the EARS (Easy Approach to Requirements Syntax) format for clarity and precision.

## Scope

This document covers the core functionality of the spec generation system.

## Requirements

### Requirement 1

**User Story:** As a user, I want feature X, so that I can achieve goal Y.

**Priority:** High

#### Acceptance Criteria

1. WHEN the user performs action A THEN the system SHALL do B
2. GIVEN condition C WHEN event D THEN outcome E
3. WHEN the user submits input THEN the system SHALL validate it

### Requirement 2

**User Story:** As an admin, I want to manage users, so that I can control access.

**Priority:** Medium

#### Acceptance Criteria

1. WHEN admin logs in THEN they SHALL see the dashboard
2. WHEN admin creates a user THEN the system SHALL send a notification
3. GIVEN a user exists WHEN admin deletes them THEN access SHALL be revoked
";

        let result = phase.postprocess(raw_response, &ctx);
        // Valid output should pass in strict mode
        assert!(result.is_ok());
    }
}
// Implementation of the Review phase
///
/// This phase reviews the generated tasks and identifies gaps or issues that need
/// to be addressed through fixups to earlier phases.
#[derive(Debug, Clone)]
pub struct ReviewPhase;

impl ReviewPhase {
    /// Create a new Review phase instance
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Phase for ReviewPhase {
    fn id(&self) -> PhaseId {
        PhaseId::Review
    }

    fn deps(&self) -> &'static [PhaseId] {
        // Review phase depends on Tasks phase
        &[PhaseId::Tasks]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, ctx: &PhaseContext) -> String {
        format!(
            r"You are a senior technical reviewer conducting a comprehensive review of the generated specification.

Your task is to review the complete specification (requirements, design, and tasks) and identify any gaps, inconsistencies, or issues that need to be addressed.

# Review Guidelines

## What to Look For:
1. **Requirements Completeness**: Are all user needs captured? Any missing edge cases?
2. **Design Consistency**: Does the design address all requirements? Any architectural gaps?
3. **Task Coverage**: Do the implementation tasks cover all design components? Any missing steps?
4. **Cross-Phase Alignment**: Are requirements, design, and tasks consistent with each other?
5. **Technical Feasibility**: Are the proposed solutions technically sound and implementable?
6. **Quality Standards**: Do the artifacts meet professional standards for clarity and completeness?

## Review Output Format:

If issues are found that require changes to earlier phases, use this format:

**FIXUP PLAN:**

[Describe the issues found and why fixups are needed]

For each file that needs changes, provide a unified diff in a fenced code block:

```diff
--- a/path/to/file
+++ b/path/to/file
@@ -start,count +start,count @@
 context line
-line to remove
+line to add
 context line
```

## If No Issues Found:

If the specification is complete and consistent, provide a summary of what was reviewed and confirm that no fixups are needed.

Spec ID: {}
Phase: Review

Please conduct a thorough review of the specification artifacts and provide your assessment.",
            ctx.spec_id
        )
    }

    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet> {
        let mut content = String::new();
        let mut files = Vec::new();

        // Add basic context information
        content.push_str("=== SPEC GENERATION CONTEXT ===\n");
        content.push_str(&format!("Spec ID: {}\n", ctx.spec_id));
        content.push_str("Phase: Review\n");
        content.push_str(&format!("Base Directory: {}\n", ctx.spec_dir.display()));
        content.push('\n');

        // Include all previous phase artifacts for comprehensive review
        content.push_str("=== COMPLETE SPECIFICATION FOR REVIEW ===\n");

        // Requirements artifacts
        let requirements_md_path = ctx.spec_dir.join("artifacts").join("00-requirements.md");
        let requirements_yaml_path = ctx
            .spec_dir
            .join("artifacts")
            .join("00-requirements.core.yaml");

        if requirements_md_path.exists() {
            match std::fs::read_to_string(&requirements_md_path) {
                Ok(requirements_content) => {
                    content.push_str("--- Requirements Document (00-requirements.md) ---\n");
                    content.push_str(&requirements_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/00-requirements.md".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(requirements_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading requirements.md: {e}\n"));
                }
            }
        }

        if requirements_yaml_path.exists() {
            match std::fs::read_to_string(&requirements_yaml_path) {
                Ok(yaml_content) => {
                    content
                        .push_str("--- Requirements Core Data (00-requirements.core.yaml) ---\n");
                    content.push_str(&yaml_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/00-requirements.core.yaml".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(yaml_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading requirements.core.yaml: {e}\n"));
                }
            }
        }

        // Design artifacts
        let design_md_path = ctx.spec_dir.join("artifacts").join("10-design.md");
        let design_yaml_path = ctx.spec_dir.join("artifacts").join("10-design.core.yaml");

        if design_md_path.exists() {
            match std::fs::read_to_string(&design_md_path) {
                Ok(design_content) => {
                    content.push_str("--- Design Document (10-design.md) ---\n");
                    content.push_str(&design_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/10-design.md".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(design_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading design.md: {e}\n"));
                }
            }
        }

        if design_yaml_path.exists() {
            match std::fs::read_to_string(&design_yaml_path) {
                Ok(yaml_content) => {
                    content.push_str("--- Design Core Data (10-design.core.yaml) ---\n");
                    content.push_str(&yaml_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/10-design.core.yaml".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(yaml_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading design.core.yaml: {e}\n"));
                }
            }
        }

        // Tasks artifacts
        let tasks_md_path = ctx.spec_dir.join("artifacts").join("20-tasks.md");
        let tasks_yaml_path = ctx.spec_dir.join("artifacts").join("20-tasks.core.yaml");

        if tasks_md_path.exists() {
            match std::fs::read_to_string(&tasks_md_path) {
                Ok(tasks_content) => {
                    content.push_str("--- Tasks Document (20-tasks.md) ---\n");
                    content.push_str(&tasks_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/20-tasks.md".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(tasks_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading tasks.md: {e}\n"));
                }
            }
        }

        if tasks_yaml_path.exists() {
            match std::fs::read_to_string(&tasks_yaml_path) {
                Ok(yaml_content) => {
                    content.push_str("--- Tasks Core Data (20-tasks.core.yaml) ---\n");
                    content.push_str(&yaml_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/20-tasks.core.yaml".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(yaml_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading tasks.core.yaml: {e}\n"));
                }
            }
        }

        // Compute hash of the packet content
        let blake3_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

        // Create evidence for the packet
        let (max_bytes, max_lines) = packet_limits_from_context(ctx);

        let evidence = PacketEvidence {
            files,
            max_bytes,
            max_lines,
        };

        let mut budget_used = BudgetUsage::new(max_bytes, max_lines);
        budget_used.add_content(content.len(), content.lines().count());

        Ok(Packet::new(content, blake3_hash, evidence, budget_used))
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        let review_content = raw.trim().to_string();

        // Create the main review.md artifact
        let review_artifact = Artifact {
            name: "30-review.md".to_string(),
            content: review_content.clone(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: blake3::hash(review_content.as_bytes()).to_hex().to_string(),
        };

        // Check if fixups are needed (simplified for now)
        let has_fixup_markers =
            review_content.contains("FIXUP PLAN:") || review_content.contains("needs fixups");
        let next_step = if has_fixup_markers {
            // Fixups are needed - proceed to Fixup phase
            NextStep::Continue
        } else {
            // No fixups needed - proceed to Final phase or complete
            NextStep::Continue
        };

        // Create a core YAML artifact with structured review data
        let core_yaml_content = self.generate_core_yaml(&review_content, ctx, has_fixup_markers)?;
        let core_yaml_artifact = Artifact {
            name: "30-review.core.yaml".to_string(),
            content: core_yaml_content.clone(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: blake3::hash(core_yaml_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let artifacts = vec![review_artifact, core_yaml_artifact];

        // Metadata will be populated by orchestrator with packet hash, budget, and duration
        let metadata = PhaseMetadata::default();

        Ok(PhaseResult {
            artifacts,
            next_step,
            metadata,
        })
    }
}

impl ReviewPhase {
    /// Generate a core YAML file with structured review data
    fn generate_core_yaml(
        &self,
        _review_md: &str,
        ctx: &PhaseContext,
        fixups_needed: bool,
    ) -> Result<String> {
        let yaml_content = format!(
            r#"# Core review data for spec {}
# This file contains structured data extracted from the review document

spec_id: "{}"
phase: "review"
version: "1.0"

# Metadata about the review
metadata:
  fixups_needed: {}
  has_fixup_plan: {}
  review_sections_found: []  # Would be parsed from markdown
  issues_identified: 0       # Would be counted from review content

# Review findings (would be extracted from markdown)
findings:
  requirements_issues: []
  design_issues: []
  tasks_issues: []
  cross_phase_issues: []

# Fixup information if needed
fixup_info:
  target_files: []           # Would be extracted from diff blocks
  change_summary: {{}}       # Would be calculated from diffs

# Generated timestamp
generated_at: "{}"
"#,
            ctx.spec_id,
            ctx.spec_id,
            fixups_needed,
            fixups_needed,
            chrono::Utc::now().to_rfc3339()
        );

        Ok(yaml_content)
    }
}

impl Default for ReviewPhase {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation of the Fixup phase
///
/// This phase applies fixups identified during the review phase to earlier
/// phase artifacts, with preview/apply modes for safety.
#[derive(Debug, Clone)]
pub struct FixupPhase {
    /// Mode for fixup operations (preview or apply)
    mode: FixupMode,
}

impl FixupPhase {
    /// Create a new Fixup phase instance with preview mode (default)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: FixupMode::Preview,
        }
    }

    /// Create a new Fixup phase instance with specified mode
    #[must_use]
    pub const fn new_with_mode(mode: FixupMode) -> Self {
        Self { mode }
    }
}

impl Phase for FixupPhase {
    fn id(&self) -> PhaseId {
        PhaseId::Fixup
    }

    fn deps(&self) -> &'static [PhaseId] {
        // Fixup phase depends on Review phase
        &[PhaseId::Review]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, ctx: &PhaseContext) -> String {
        format!(
            r"You are a fixup specialist responsible for applying the changes identified in the review phase.

Your task is to process the review findings and apply the necessary fixups to the specification artifacts.

# Fixup Process

## Input Analysis:
- Review the fixup plan from the review phase
- Validate the proposed changes against the current artifacts
- Ensure changes maintain consistency across all phases

## Change Application:
- Apply changes according to the unified diffs provided
- Update affected artifacts while preserving their structure
- Maintain cross-references and dependencies between phases

## Validation:
- Verify that applied changes resolve the identified issues
- Ensure no new inconsistencies are introduced
- Confirm that all artifacts remain well-formed

Spec ID: {}
Phase: Fixup

The review phase has identified issues that need to be addressed. Please process the fixup plan and apply the necessary changes.",
            ctx.spec_id
        )
    }

    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet> {
        let mut content = String::new();
        let mut files = Vec::new();

        // Add basic context information
        content.push_str("=== SPEC GENERATION CONTEXT ===\n");
        content.push_str(&format!("Spec ID: {}\n", ctx.spec_id));
        content.push_str("Phase: Fixup\n");
        content.push_str(&format!("Base Directory: {}\n", ctx.spec_dir.display()));
        content.push('\n');

        // Include the review document with fixup plan
        content.push_str("=== REVIEW AND FIXUP PLAN ===\n");

        let review_md_path = ctx.spec_dir.join("artifacts").join("30-review.md");
        if review_md_path.exists() {
            match std::fs::read_to_string(&review_md_path) {
                Ok(review_content) => {
                    content.push_str("--- Review Document with Fixup Plan (30-review.md) ---\n");
                    content.push_str(&review_content);
                    content.push_str("\n\n");

                    files.push(crate::types::FileEvidence {
                        path: "artifacts/30-review.md".to_string(),
                        range: None,
                        blake3_pre_redaction: blake3::hash(review_content.as_bytes())
                            .to_hex()
                            .to_string(),
                        priority: crate::types::Priority::Upstream,
                    });
                }
                Err(e) => {
                    content.push_str(&format!("Error reading review.md: {e}\n"));
                }
            }
        }

        // Include current artifacts that may need fixing
        content.push_str("=== CURRENT ARTIFACTS TO BE FIXED ===\n");

        // Include all previous phase artifacts
        let artifact_paths = [
            ("00-requirements.md", "Requirements Document"),
            ("00-requirements.core.yaml", "Requirements Core Data"),
            ("10-design.md", "Design Document"),
            ("10-design.core.yaml", "Design Core Data"),
            ("20-tasks.md", "Tasks Document"),
            ("20-tasks.core.yaml", "Tasks Core Data"),
        ];

        for (filename, description) in &artifact_paths {
            let artifact_path = ctx.spec_dir.join("artifacts").join(filename);
            if artifact_path.exists() {
                match std::fs::read_to_string(&artifact_path) {
                    Ok(artifact_content) => {
                        content.push_str(&format!("--- {description} ({filename}) ---\n"));
                        content.push_str(&artifact_content);
                        content.push_str("\n\n");

                        files.push(crate::types::FileEvidence {
                            path: format!("artifacts/{filename}"),
                            range: None,
                            blake3_pre_redaction: blake3::hash(artifact_content.as_bytes())
                                .to_hex()
                                .to_string(),
                            priority: crate::types::Priority::Upstream,
                        });
                    }
                    Err(e) => {
                        content.push_str(&format!("Error reading {filename}: {e}\n"));
                    }
                }
            }
        }

        // Compute hash of the packet content
        let blake3_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

        // Create evidence for the packet
        let (max_bytes, max_lines) = packet_limits_from_context(ctx);

        let evidence = PacketEvidence {
            files,
            max_bytes,
            max_lines,
        };

        let mut budget_used = BudgetUsage::new(max_bytes, max_lines);
        budget_used.add_content(content.len(), content.lines().count());

        Ok(Packet::new(content, blake3_hash, evidence, budget_used))
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        let fixup_content = raw.trim().to_string();

        // Create the main fixup.md artifact
        let fixup_artifact = Artifact {
            name: "40-fixup.md".to_string(),
            content: fixup_content.clone(),
            artifact_type: ArtifactType::Markdown,
            blake3_hash: blake3::hash(fixup_content.as_bytes()).to_hex().to_string(),
        };

        let mut metadata = HashMap::new();
        let mut next_step = NextStep::Continue;

        // Create fixup parser with the configured mode
        let fixup_parser = FixupParser::new(self.mode, ctx.spec_dir.clone())?;

        // Process fixups if markers are detected
        if fixup_parser.has_fixup_markers(&fixup_content) {
            match fixup_parser.parse_diffs(&fixup_content) {
                Ok(diffs) => {
                    metadata.insert("fixup_diffs_found".to_string(), diffs.len().to_string());

                    // Handle preview vs apply mode
                    match self.mode {
                        FixupMode::Preview => {
                            // Preview mode: validate and list targets
                            match fixup_parser.preview_changes(&diffs) {
                                Ok(preview) => {
                                    metadata
                                        .insert("fixup_mode".to_string(), "preview".to_string());
                                    metadata.insert(
                                        "target_files_count".to_string(),
                                        preview.target_files.len().to_string(),
                                    );
                                    metadata.insert(
                                        "all_valid".to_string(),
                                        preview.all_valid.to_string(),
                                    );

                                    // Add target files to metadata
                                    for (i, target) in preview.target_files.iter().enumerate() {
                                        metadata.insert(format!("target_file_{i}"), target.clone());
                                    }

                                    // Add warnings to metadata
                                    for (i, warning) in preview.warnings.iter().enumerate() {
                                        metadata.insert(format!("warning_{i}"), warning.clone());
                                    }
                                }
                                Err(e) => {
                                    metadata
                                        .insert("fixup_preview_error".to_string(), e.to_string());
                                }
                            }
                        }
                        FixupMode::Apply => {
                            // Apply mode: actually apply the changes
                            match fixup_parser.apply_changes(&diffs) {
                                Ok(result) => {
                                    metadata.insert("fixup_mode".to_string(), "apply".to_string());
                                    metadata.insert(
                                        "applied_files_count".to_string(),
                                        result.applied_files.len().to_string(),
                                    );
                                    metadata.insert(
                                        "failed_files_count".to_string(),
                                        result.failed_files.len().to_string(),
                                    );
                                    metadata.insert(
                                        "three_way_used".to_string(),
                                        result.three_way_used.to_string(),
                                    );

                                    // Add applied files to metadata
                                    for (i, applied) in result.applied_files.iter().enumerate() {
                                        metadata.insert(
                                            format!("applied_file_{i}"),
                                            format!(
                                                "{} (blake3: {}, applied: {})",
                                                applied.path,
                                                applied.blake3_first8,
                                                applied.applied
                                            ),
                                        );
                                    }

                                    // Add failed files to metadata
                                    for (i, failed) in result.failed_files.iter().enumerate() {
                                        metadata.insert(format!("failed_file_{i}"), failed.clone());
                                    }

                                    // Add warnings to metadata
                                    for (i, warning) in result.warnings.iter().enumerate() {
                                        metadata.insert(format!("warning_{i}"), warning.clone());
                                    }

                                    // Determine if rewind is needed based on applied files
                                    if result
                                        .applied_files
                                        .iter()
                                        .any(|f| f.path.contains("requirements"))
                                    {
                                        next_step = NextStep::Rewind {
                                            to: PhaseId::Requirements,
                                        };
                                    } else if result
                                        .applied_files
                                        .iter()
                                        .any(|f| f.path.contains("design"))
                                    {
                                        next_step = NextStep::Rewind {
                                            to: PhaseId::Design,
                                        };
                                    } else if result
                                        .applied_files
                                        .iter()
                                        .any(|f| f.path.contains("tasks"))
                                    {
                                        next_step = NextStep::Rewind { to: PhaseId::Tasks };
                                    }
                                }
                                Err(e) => {
                                    metadata.insert("fixup_apply_error".to_string(), e.to_string());
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    metadata.insert("fixup_parse_error".to_string(), e.to_string());
                }
            }
        } else {
            metadata.insert("fixup_markers_found".to_string(), "false".to_string());
        }

        // Create a core YAML artifact with structured fixup data
        let core_yaml_content = self.generate_core_yaml(&fixup_content, ctx, &metadata)?;
        let core_yaml_artifact = Artifact {
            name: "40-fixup.core.yaml".to_string(),
            content: core_yaml_content.clone(),
            artifact_type: ArtifactType::CoreYaml,
            blake3_hash: blake3::hash(core_yaml_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let artifacts = vec![fixup_artifact, core_yaml_artifact];

        // Metadata will be populated by orchestrator with packet hash, budget, and duration
        // (The local 'metadata' HashMap is used only for generating core YAML)
        let phase_metadata = PhaseMetadata::default();

        Ok(PhaseResult {
            artifacts,
            next_step,
            metadata: phase_metadata,
        })
    }
}

impl FixupPhase {
    /// Generate a core YAML file with structured fixup data
    fn generate_core_yaml(
        &self,
        _fixup_md: &str,
        ctx: &PhaseContext,
        metadata: &HashMap<String, String>,
    ) -> Result<String> {
        let default_zero = "0".to_string();
        let default_false = "false".to_string();
        let default_preview = "preview".to_string();

        let has_diffs = metadata.get("fixup_diffs_found").unwrap_or(&default_zero) != "0";
        let target_files_count = metadata.get("target_files_count").unwrap_or(&default_zero);
        let mode = metadata.get("fixup_mode").unwrap_or(&default_preview);
        let all_valid = metadata.get("all_valid").unwrap_or(&default_false);

        let yaml_content = format!(
            r#"# Core fixup data for spec {}
# This file contains structured data extracted from the fixup document

spec_id: "{}"
phase: "fixup"
version: "1.0"

# Metadata about the fixup
metadata:
  has_diffs: {}
  target_files_count: {}
  mode: "{}"
  validation_passed: {}

# Fixup execution results
execution:
  applied_files_count: {}
  failed_files_count: {}
  three_way_used: {}

# Generated timestamp
generated_at: "{}"
"#,
            ctx.spec_id,
            ctx.spec_id,
            has_diffs,
            target_files_count,
            mode,
            all_valid,
            metadata.get("applied_files_count").unwrap_or(&default_zero),
            metadata.get("failed_files_count").unwrap_or(&default_zero),
            metadata.get("three_way_used").unwrap_or(&default_false),
            chrono::Utc::now().to_rfc3339()
        );

        Ok(yaml_content)
    }
}

impl Default for FixupPhase {
    fn default() -> Self {
        Self::new()
    }
}
