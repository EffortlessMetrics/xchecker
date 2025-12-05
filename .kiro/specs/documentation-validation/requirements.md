# Requirements Document

## Introduction

This spec covers comprehensive validation and updating of all xchecker documentation to ensure accuracy, completeness, and alignment with the current implementation. The goal is to verify that all documentation (README, guides, schemas, examples) correctly describes the actual behavior of the system.

## Glossary

- **xchecker**: The Rust CLI tool for orchestrating spec generation workflows
- **JCS**: JSON Canonicalization Scheme (RFC 8785) - deterministic JSON serialization
- **Schema Example**: JSON payload examples (minimal/full) that demonstrate schema usage
- **CLI Option**: Command-line flag or argument accepted by xchecker commands
- **Contract**: Versioned JSON output format (receipt, status, doctor)
- **XCHECKER_HOME**: Environment variable for overriding the default state directory

## Requirements

### Requirement 1

**User Story:** As a developer, I want the README to accurately document all commands and options, so that I can use xchecker without confusion.

#### Acceptance Criteria

1. WHEN the README documents a command THEN the system SHALL parse README command headers and verify each command exists in the CLI via clap's command registry
2. WHEN the README documents a CLI option THEN the system SHALL extract documented options and verify they are accepted by parsing clap's help output or command structure
3. WHEN the README shows example commands THEN the system SHALL execute them in stub mode with XCHECKER_HOME isolation and verify exit code 0
4. WHEN the README describes command behavior THEN the system SHALL verify specific side effects (files created, JSON fields present, array sorting) match documented behavior
5. WHEN the README lists exit codes THEN the system SHALL verify the exit code table matches the constants in the exit_codes module exactly

### Requirement 2

**User Story:** As an API consumer, I want schema examples to be valid and complete, so that I can trust them as integration references.

#### Acceptance Criteria

1. WHEN schema examples exist THEN the system SHALL validate them against their corresponding JSON schemas using the jsonschema crate
2. WHEN minimal examples exist THEN they SHALL include only non-Option fields from the Rust struct
3. WHEN full examples exist THEN they SHALL include all fields (required + all Option<T> with representative values) via make_example_*_full() constructors
4. WHEN examples are generated THEN they SHALL use actual struct constructors (make_example_receipt_minimal/full, make_example_status_minimal/full, make_example_doctor_minimal/full) and be written to docs/schemas/*.json
5. WHEN examples include arrays THEN the system SHALL assert arrays are sorted (outputs by path, artifacts by path, checks by name) before emission

### Requirement 3

**User Story:** As a developer, I want configuration documentation to match the actual config structure, so that I can configure xchecker correctly.

#### Acceptance Criteria

1. WHEN CONFIGURATION.md documents a config field THEN the system SHALL extract field names from TOML examples and verify each exists in the Config struct via serde field list
2. WHEN CONFIGURATION.md shows example TOML THEN the system SHALL extract TOML code fences and parse them with toml::from_str into Config without error
3. WHEN CONFIGURATION.md describes precedence THEN the system SHALL test with defaults + file + CLI inputs and assert effective_config.source values are "cli" > "config" > "default" in that order
4. WHEN CONFIGURATION.md lists default values THEN the system SHALL compare documented defaults against Config::default() field values and assert equality

### Requirement 4

**User Story:** As an operator, I want DOCTOR.md to accurately describe health checks, so that I can troubleshoot environment issues.

#### Acceptance Criteria

1. WHEN DOCTOR.md lists a health check THEN the system SHALL parse the documented check names and verify each appears in xchecker doctor --json output
2. WHEN DOCTOR.md describes check behavior THEN the system SHALL run doctor in stub mode and assert specific check.status values match documented outcomes
3. WHEN DOCTOR.md shows example output THEN the system SHALL validate the example JSON against schemas/doctor.v1.json
4. WHEN DOCTOR.md describes exit behavior THEN the system SHALL force a CheckStatus::Fail via environment flag and assert process exit != 0

### Requirement 5

**User Story:** As an API consumer, I want CONTRACTS.md to accurately describe the versioning policy, so that I can plan for schema changes.

#### Acceptance Criteria

1. WHEN CONTRACTS.md describes JCS emission THEN the system SHALL verify all contracts use JCS
2. WHEN CONTRACTS.md lists array sorting rules THEN the system SHALL verify arrays are sorted before emission
3. WHEN CONTRACTS.md describes the deprecation policy THEN the system SHALL verify it matches the implementation approach
4. WHEN CONTRACTS.md lists schema files THEN the system SHALL verify those files exist

### Requirement 6

**User Story:** As a developer, I want JSON schemas to be valid and complete, so that I can validate outputs programmatically.

#### Acceptance Criteria

1. WHEN a JSON schema exists THEN the system SHALL parse it with jsonschema crate and verify it is valid JSON Schema Draft 7 or later
2. WHEN a schema defines field constraints THEN the system SHALL compare schema properties against Rust struct fields via serde introspection
3. WHEN a schema includes enums THEN the system SHALL extract schema enum values and compare against Rust enum variants with #[serde(rename_all)] applied
4. WHEN a schema defines required fields THEN the system SHALL compare schema required array against non-Option<T> fields in the corresponding Rust struct

### Requirement 7

**User Story:** As a developer, I want the CHANGELOG to document all user-facing changes, so that I can understand version differences.

#### Acceptance Criteria

1. WHEN new fields are added to contracts THEN the system SHALL verify CHANGELOG contains those field names in a dated entry
2. WHEN exit codes are added or changed THEN the system SHALL verify CHANGELOG documents the exit code number and name
3. WHEN CLI options are added or changed THEN the system SHALL verify CHANGELOG mentions the option flag name
4. WHEN breaking changes occur THEN the system SHALL verify CHANGELOG contains a "Breaking Changes" heading or [BREAKING] marker for that version

### Requirement 8

**User Story:** As a developer, I want documentation to accurately describe XCHECKER_HOME behavior, so that I can control state location.

#### Acceptance Criteria

1. WHEN documentation mentions state location THEN it SHALL describe XCHECKER_HOME override
2. WHEN documentation shows directory structure THEN it SHALL match the actual paths module
3. WHEN documentation describes test isolation THEN it SHALL mention thread-local override
4. WHEN documentation shows default location THEN it SHALL be `./.xchecker`

### Requirement 9

**User Story:** As a developer, I want all code examples in documentation to be correct, so that I can copy-paste them without errors.

#### Acceptance Criteria

1. WHEN documentation includes shell commands THEN the system SHALL extract ```bash or ```sh fenced blocks and execute them in stub mode with XCHECKER_HOME isolation, asserting exit code 0
2. WHEN documentation includes TOML config THEN the system SHALL extract ```toml fenced blocks and parse them with toml::from_str, asserting no parse errors
3. WHEN documentation includes JSON examples THEN the system SHALL extract ```json fenced blocks and validate them against the appropriate schema using jsonschema crate
4. WHEN documentation includes jq commands THEN the system SHALL execute them against generated example outputs using jq binary or jql crate, asserting successful execution

### Requirement 10

**User Story:** As a contributor, I want documentation to describe the actual implementation, so that I can understand the system correctly.

#### Acceptance Criteria

1. WHEN documentation describes a feature (timeout, lockfile, fixup) THEN the system SHALL verify a corresponding smoke test exists and passes
2. WHEN documentation describes behavior THEN the system SHALL run the feature and assert specific side effects (files created, exit codes, JSON fields) match documentation
3. WHEN documentation lists constraints (path validation, array sorting, JCS emission) THEN the system SHALL verify tests exist that enforce those constraints
4. WHEN documentation shows output format THEN the system SHALL generate actual output and compare structure/fields against documented examples using JSON schema validation

## Non-Functional Requirements

**NFR1 Accuracy:** All documentation must accurately reflect the current implementation

**NFR2 Completeness:** All public features must be documented

**NFR3 Consistency:** Documentation must use consistent terminology and formatting

**NFR4 Testability:** Documentation claims must be verifiable through automated tests
