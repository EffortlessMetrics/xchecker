//! Minimal metadata extraction from phase outputs
//!
//! This module provides B3.0 (minimal extraction) functionality to populate
//! `.core.yaml` files with real metadata from markdown artifacts.
//!
//! # Design Philosophy
//!
//! B3.0 focuses on cheap, robust metadata extraction without full AST parsing:
//! - Count well-formed patterns reliably
//! - Avoid complex parsing that might break on edge cases
//! - Provide useful metrics for dashboards and gate checks
//!
//! Future B3.1 will add structured extraction of full requirement/design objects.

use regex::Regex;

/// Summary statistics extracted from a requirements markdown document
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RequirementsSummary {
    /// Number of requirements found (### Requirement N headings)
    pub requirement_count: usize,
    /// Number of user stories found (**User Story:** patterns)
    pub user_story_count: usize,
    /// Number of acceptance criteria found (EARS-style WHEN/THEN/SHALL)
    pub acceptance_criteria_count: usize,
    /// Number of non-functional requirements found (**NFR patterns)
    pub nfr_count: usize,
}

/// Summary statistics extracted from a design markdown document
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DesignSummary {
    /// Number of components defined (## Component or ### Component headings)
    pub component_count: usize,
    /// Number of interfaces defined (## Interface or ### Interface patterns)
    pub interface_count: usize,
    /// Number of data models defined (## Data Model or similar patterns)
    pub data_model_count: usize,
    /// Whether the document has an architecture section
    pub has_architecture: bool,
    /// Whether the document has mermaid diagrams
    pub has_diagrams: bool,
}

/// Summary statistics extracted from a tasks markdown document
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TasksSummary {
    /// Number of tasks found (## Task N or ### Task patterns)
    pub task_count: usize,
    /// Number of subtasks found (checkbox items)
    pub subtask_count: usize,
    /// Number of milestones found (## Milestone or similar)
    pub milestone_count: usize,
    /// Number of dependencies mentioned
    pub dependency_count: usize,
}

/// Extract summary metadata from a requirements markdown document
///
/// Uses simple regex patterns to count well-formed requirements elements.
/// Designed to be robust against minor formatting variations.
///
/// # Examples
///
/// ```ignore
/// // Note: This doctest is marked ignore due to raw string handling in doc comments.
/// // See unit tests below for equivalent coverage.
/// use xchecker_engine::extraction::summarize_requirements;
///
/// let markdown = "### Requirement 1\n**User Story:** As a user...\n";
/// let summary = summarize_requirements(markdown);
/// assert_eq!(summary.requirement_count, 1);
/// ```
#[must_use]
pub fn summarize_requirements(markdown: &str) -> RequirementsSummary {
    let mut summary = RequirementsSummary::default();

    // Match user story patterns: **User Story:** or **User Story**:
    let user_story_re = Regex::new(r"(?im)^\s*\*\*User\s+Story[:\*]").unwrap();
    summary.user_story_count = user_story_re.find_iter(markdown).count();

    // Match EARS-style acceptance criteria: WHEN ... THEN ... SHALL
    // Also match simpler patterns: GIVEN/WHEN/THEN or numbered criteria with SHALL
    let ears_re =
        Regex::new(r"(?im)(WHEN\s+.+\s+THEN\s+.+\s+SHALL|GIVEN\s+.+\s+WHEN\s+.+\s+THEN)").unwrap();
    summary.acceptance_criteria_count = ears_re.find_iter(markdown).count();

    // Match NFR patterns: **NFR-*, NFR-*, **Non-Functional*
    let nfr_re = Regex::new(r"(?im)(^\s*\*\*NFR[-\s]|\bNFR-\w+\b|^\s*\*\*Non-Functional)").unwrap();
    summary.nfr_count = nfr_re.find_iter(markdown).count();

    // Match requirement headings: ### Requirement N or ## Requirement N (allow leading whitespace)
    let req_heading_re = Regex::new(r"(?im)^\s*#{2,3}\s+Requirement\s+\d+").unwrap();
    summary.requirement_count = req_heading_re.find_iter(markdown).count();

    // If no formal requirement headings, try to count by user story count
    if summary.requirement_count == 0 && summary.user_story_count > 0 {
        summary.requirement_count = summary.user_story_count;
    }

    summary
}

/// Extract summary metadata from a design markdown document
///
/// Uses simple regex patterns to count well-formed design elements.
///
/// # Examples
///
/// ```ignore
/// // Note: This doctest is marked ignore due to raw string handling in doc comments.
/// // See unit tests below for equivalent coverage.
/// use xchecker_engine::extraction::summarize_design;
///
/// let markdown = "## Architecture\n### Component: AuthService\n";
/// let summary = summarize_design(markdown);
/// assert!(summary.has_architecture);
/// assert_eq!(summary.component_count, 1);
/// ```
#[must_use]
pub fn summarize_design(markdown: &str) -> DesignSummary {
    let mut summary = DesignSummary::default();

    // Check for architecture section (allow leading whitespace from indented doc content)
    let arch_re = Regex::new(r"(?im)^\s*#{1,3}\s+(Architecture|System\s+Architecture)").unwrap();
    summary.has_architecture = arch_re.is_match(markdown);

    // Check for mermaid diagrams
    summary.has_diagrams = markdown.contains("```mermaid");

    // Count components: ### Component: X or ## Component: X or ### X Component
    let component_re =
        Regex::new(r"(?im)^\s*#{2,3}\s+(Component[:\s]|[A-Z]\w+\s+Component)").unwrap();
    summary.component_count = component_re.find_iter(markdown).count();

    // Count interfaces: ### Interface: X or ## Interface: X or ### X API
    let interface_re =
        Regex::new(r"(?im)^\s*#{2,3}\s+(Interface[:\s]|[A-Z]\w+\s+(API|Interface))").unwrap();
    summary.interface_count = interface_re.find_iter(markdown).count();

    // Count data models: ## Data Model or ### Model: or ### Schema:
    let model_re =
        Regex::new(r"(?im)^\s*#{2,3}\s+(Data\s+Model|Model[:\s]|Schema[:\s]|Entity[:\s])").unwrap();
    summary.data_model_count = model_re.find_iter(markdown).count();

    summary
}

/// Extract summary metadata from a tasks markdown document
///
/// Uses simple regex patterns to count well-formed task elements.
///
/// # Examples
///
/// ```ignore
/// // Note: This doctest is marked ignore due to raw string handling in doc comments.
/// // See unit tests below for equivalent coverage.
/// use xchecker_engine::extraction::summarize_tasks;
///
/// let markdown = "## Task 1\n- [x] Done\n## Milestone 1\n";
/// let summary = summarize_tasks(markdown);
/// assert_eq!(summary.task_count, 1);
/// assert_eq!(summary.milestone_count, 1);
/// ```
#[must_use]
pub fn summarize_tasks(markdown: &str) -> TasksSummary {
    let mut summary = TasksSummary::default();

    // Count tasks: ## Task N or ### Task N (allow leading whitespace from indented doc content)
    let task_re = Regex::new(r"(?im)^\s*#{2,3}\s+Task\s+\d+").unwrap();
    summary.task_count = task_re.find_iter(markdown).count();

    // Count subtasks: checkbox items - [ ] or - [x]
    let subtask_re = Regex::new(r"(?m)^\s*[-*]\s+\[[ xX]\]").unwrap();
    summary.subtask_count = subtask_re.find_iter(markdown).count();

    // Count milestones: ## Milestone or ### Milestone
    let milestone_re = Regex::new(r"(?im)^\s*#{2,3}\s+Milestone").unwrap();
    summary.milestone_count = milestone_re.find_iter(markdown).count();

    // Count dependencies: "Depends on:" or "Dependencies:"
    let dep_re = Regex::new(r"(?im)(Depends\s+on:|Dependencies:)").unwrap();
    summary.dependency_count = dep_re.find_iter(markdown).count();

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_requirements_empty() {
        let summary = summarize_requirements("");
        assert_eq!(summary.requirement_count, 0);
        assert_eq!(summary.user_story_count, 0);
        assert_eq!(summary.acceptance_criteria_count, 0);
        assert_eq!(summary.nfr_count, 0);
    }

    #[test]
    fn test_summarize_requirements_with_user_stories() {
        let md = r#"
### Requirement 1

**User Story:** As a user, I want to do X.

### Requirement 2

**User Story:** As an admin, I want to do Y.
"#;
        let summary = summarize_requirements(md);
        assert_eq!(summary.requirement_count, 2);
        assert_eq!(summary.user_story_count, 2);
    }

    #[test]
    fn test_summarize_requirements_ears_criteria() {
        let md = r#"
1. WHEN a user clicks login THEN the system SHALL display a form
2. WHEN credentials are valid THEN the system SHALL grant access
3. GIVEN a logged in user WHEN they click logout THEN session ends
"#;
        let summary = summarize_requirements(md);
        assert_eq!(summary.acceptance_criteria_count, 3);
    }

    #[test]
    fn test_summarize_requirements_nfrs() {
        let md = r#"
**NFR-SEC-001:** Passwords must be hashed
**NFR-PERF-002:** Response time < 200ms
NFR-AVAIL-003: 99.9% uptime
**Non-Functional Requirements:**
"#;
        let summary = summarize_requirements(md);
        assert_eq!(summary.nfr_count, 4);
    }

    #[test]
    fn test_summarize_requirements_infers_from_user_stories() {
        // No formal ### Requirement N headings, but has user stories
        let md = r#"
**User Story:** First story
**User Story:** Second story
"#;
        let summary = summarize_requirements(md);
        assert_eq!(summary.requirement_count, 2); // Inferred from user stories
        assert_eq!(summary.user_story_count, 2);
    }

    #[test]
    fn test_summarize_design_empty() {
        let summary = summarize_design("");
        assert!(!summary.has_architecture);
        assert!(!summary.has_diagrams);
        assert_eq!(summary.component_count, 0);
        assert_eq!(summary.interface_count, 0);
        assert_eq!(summary.data_model_count, 0);
    }

    #[test]
    fn test_summarize_design_with_architecture() {
        let md = "## Architecture\n\nDescription here.";
        let summary = summarize_design(md);
        assert!(summary.has_architecture);
    }

    #[test]
    fn test_summarize_design_with_mermaid() {
        let md = "```mermaid\ngraph TD\n```";
        let summary = summarize_design(md);
        assert!(summary.has_diagrams);
    }

    #[test]
    fn test_summarize_design_components() {
        let md = r#"
### Component: AuthService
### Component: UserService
## Component: DataService
"#;
        let summary = summarize_design(md);
        assert_eq!(summary.component_count, 3);
    }

    #[test]
    fn test_summarize_tasks_empty() {
        let summary = summarize_tasks("");
        assert_eq!(summary.task_count, 0);
        assert_eq!(summary.subtask_count, 0);
        assert_eq!(summary.milestone_count, 0);
        assert_eq!(summary.dependency_count, 0);
    }

    #[test]
    fn test_summarize_tasks_full() {
        let md = r#"
## Milestone 1

## Task 1

- [x] Done
- [ ] Todo

## Task 2

Depends on: Task 1

- [ ] Another todo
"#;
        let summary = summarize_tasks(md);
        assert_eq!(summary.task_count, 2);
        assert_eq!(summary.subtask_count, 3);
        assert_eq!(summary.milestone_count, 1);
        assert_eq!(summary.dependency_count, 1);
    }
}
