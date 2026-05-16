//! Dry-run LLM response simulation for phase execution.

use crate::types::PhaseId;

use super::PhaseOrchestrator;

impl PhaseOrchestrator {
    /// Create a simulated LlmResult for dry-run mode
    /// This ensures receipts have complete LLM metadata even during testing
    pub(crate) fn simulate_llm_result(&self, _phase_id: PhaseId) -> crate::llm::LlmResult {
        crate::llm::LlmResult::new(
            "simulated response".to_string(),
            "claude-cli-simulated".to_string(),
            "haiku".to_string(),
        )
        .with_tokens(1000, 2000)
        .with_timeout(false)
        .with_timeout_seconds(600) // Default timeout for simulated runs
        .with_extension("dry_run", serde_json::json!(true))
    }

    /// Simulate Claude CLI response for testing/dry-run
    pub(crate) fn simulate_claude_response(&self, _phase_id: PhaseId, _prompt: &str) -> String {
        match _phase_id {
            PhaseId::Requirements => {
                // Generate a realistic requirements document
                r"# Requirements Document

## Introduction

This is a generated requirements document for the current specification. The system will provide core functionality for managing and processing specifications through a structured workflow.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to generate structured requirements from rough ideas, so that I can create comprehensive specifications efficiently.

#### Acceptance Criteria

1. WHEN I provide a problem statement THEN the system SHALL generate structured requirements in EARS format
2. WHEN requirements are generated THEN they SHALL include user stories and acceptance criteria
3. WHEN the process completes THEN the system SHALL produce both markdown and YAML artifacts

### Requirement 2

**User Story:** As a developer, I want deterministic output generation, so that I can reproduce results consistently.

#### Acceptance Criteria

1. WHEN identical inputs are provided THEN the system SHALL produce identical canonicalized outputs
2. WHEN artifacts are created THEN they SHALL include BLAKE3 hashes for verification
3. WHEN the process runs THEN it SHALL create audit receipts for traceability

### Requirement 3

**User Story:** As a developer, I want atomic file operations, so that partial writes don't corrupt the system state.

#### Acceptance Criteria

1. WHEN writing artifacts THEN the system SHALL use atomic write operations
2. WHEN failures occur THEN partial artifacts SHALL be preserved for debugging
3. WHEN operations complete THEN all files SHALL be in a consistent state

## Non-Functional Requirements

**NFR1 Performance:** The system SHALL complete requirements generation within reasonable time limits
**NFR2 Reliability:** All file operations SHALL be atomic to prevent corruption
**NFR3 Auditability:** All operations SHALL be logged with cryptographic verification
".to_string()
            }
            PhaseId::Design => {
                r"# Design Document

## Overview

This is a comprehensive design document for the current specification. The system implements a phase-based architecture for orchestrating spec generation workflows using the Claude CLI.

## Architecture

The system follows a modular architecture with clear separation of concerns:

```mermaid
graph TD
    A[CLI Entry] --> B[Phase Orchestrator]
    B --> C[Requirements Phase]
    C --> D[Design Phase]
    D --> E[Tasks Phase]
    E --> F[Review Phase]
```

## Components and Interfaces

### Phase System
- **Phase trait**: Defines the interface for all workflow phases
- **PhaseOrchestrator**: Manages phase execution and dependencies
- **PhaseContext**: Provides context and configuration to phases

### Artifact Management
- **ArtifactManager**: Handles atomic file operations and storage
- **ReceiptManager**: Creates and manages execution receipts
- **Canonicalizer**: Ensures deterministic output formatting

## Data Models

### Core Types
- `PhaseId`: Enumeration of available phases
- `Artifact`: Represents generated outputs with metadata
- `Receipt`: Audit trail for phase execution

### Configuration
- `OrchestratorConfig`: Runtime configuration parameters
- `PhaseContext`: Execution context for phases

## Error Handling

The system implements comprehensive error handling with:
- Structured error types for different failure modes
- Partial artifact preservation on failures
- Detailed error reporting with context

## Testing Strategy

- Unit tests for individual components
- Integration tests for end-to-end workflows
- Property-based tests for determinism validation
- Mock Claude CLI for testing scenarios
".to_string()
            }
            PhaseId::Tasks => {
                r"# Implementation Plan

## Milestone 1: Core Phase System

- [ ] 1. Set up project structure and core interfaces
  - Create directory structure for phases, artifacts, and receipts
  - Define Phase trait with separated concerns (prompt, make_packet, postprocess)
  - Implement PhaseId enum and basic dependency system
  - _Requirements: R10.1, R10.3_

- [ ] 2. Implement Requirements phase
- [ ] 2.1 Create RequirementsPhase struct
  - Implement Phase trait methods for requirements generation
  - Create prompt template for EARS format requirements
  - Add packet construction with basic context
  - _Requirements: R1.1_

- [ ] 2.2 Add requirements postprocessing
  - Parse Claude response into requirements.md artifact
  - Generate requirements.core.yaml with structured data
  - Implement artifact creation and storage
  - _Requirements: R1.1, R2.1_

- [ ]* 2.3 Write unit tests for Requirements phase
  - Test prompt generation and packet creation
  - Verify postprocessing creates correct artifacts
  - Test error handling scenarios
  - _Requirements: R1.1_

## Milestone 2: Design and Tasks Phases

- [ ] 3. Implement Design phase
- [ ] 3.1 Create DesignPhase struct
  - Implement Phase trait with architecture-focused prompts
  - Add dependency on Requirements phase
  - Include requirements artifacts in packet construction
  - _Requirements: R1.1_

- [ ] 3.2 Add design postprocessing
  - Parse Claude response into design.md artifact
  - Generate design.core.yaml with structured data
  - Implement component and interface extraction
  - _Requirements: R1.1, R2.1_

- [ ] 4. Implement Tasks phase
- [ ] 4.1 Create TasksPhase struct
  - Implement Phase trait with implementation planning prompts
  - Add dependencies on Design and Requirements phases
  - Include all upstream artifacts in packet construction
  - _Requirements: R1.1_

- [ ] 4.2 Add tasks postprocessing
  - Parse Claude response into tasks.md artifact
  - Generate tasks.core.yaml with structured task data
  - Implement task parsing and validation
  - _Requirements: R1.1, R2.1_

- [ ]* 4.3 Write integration tests for phase system
  - Test Requirements → Design → Tasks flow
  - Verify dependency checking works correctly
  - Test artifact propagation between phases
  - _Requirements: R1.1, R4.2_

## Milestone 3: Orchestrator Integration

- [ ] 5. Update PhaseOrchestrator for new phases
- [ ] 5.1 Add execution methods for Design and Tasks phases
  - Implement execute_design_phase method
  - Implement execute_tasks_phase method
  - Update dependency checking logic
  - _Requirements: R1.1, R4.2_

- [ ] 5.2 Enhance Claude response simulation
  - Add realistic responses for Design phase
  - Add realistic responses for Tasks phase
  - Update test scenarios for all phases
  - _Requirements: R4.1_

- [ ]* 5.3 Write end-to-end integration tests
  - Test complete Requirements → Design → Tasks workflow
  - Verify artifact creation and receipt generation
  - Test error handling and partial artifact storage
  - _Requirements: R1.1, R4.3_
".to_string()
            }
            _ => {
                format!("Simulated response for phase: {}", _phase_id.as_str())
            }
        }
    }
}
