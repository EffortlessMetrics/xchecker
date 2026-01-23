//! Spec templates for xchecker
//!
//! This module provides built-in templates for bootstrapping specs quickly.
//! Templates include predefined problem statements, configuration, and example
//! partial spec flows for common use cases.
//!
//! Requirements:
//! - 4.7.1: `xchecker template list` lists built-in templates
//! - 4.7.2: `xchecker template init <template> <spec-id>` seeds spec from template
//! - 4.7.3: Each template has a README describing intended use

use crate::atomic_write::write_file_atomic;
use anyhow::{Context, Result};
use camino::Utf8Path;

/// Built-in template identifiers
pub const TEMPLATE_FULLSTACK_NEXTJS: &str = "fullstack-nextjs";
pub const TEMPLATE_RUST_MICROSERVICE: &str = "rust-microservice";
pub const TEMPLATE_PYTHON_FASTAPI: &str = "python-fastapi";
pub const TEMPLATE_DOCS_REFACTOR: &str = "docs-refactor";

/// All available built-in templates
pub const BUILT_IN_TEMPLATES: &[&str] = &[
    TEMPLATE_FULLSTACK_NEXTJS,
    TEMPLATE_RUST_MICROSERVICE,
    TEMPLATE_PYTHON_FASTAPI,
    TEMPLATE_DOCS_REFACTOR,
];

/// Template metadata
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Template identifier
    pub id: &'static str,
    /// Human-readable name
    pub name: &'static str,
    /// Short description
    pub description: &'static str,
    /// Intended use case
    pub use_case: &'static str,
    /// Prerequisites
    pub prerequisites: &'static [&'static str],
}

/// Get metadata for all built-in templates
#[must_use]
pub fn list_templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo {
            id: TEMPLATE_FULLSTACK_NEXTJS,
            name: "Full-Stack Next.js",
            description: "Template for full-stack web applications using Next.js",
            use_case: "Building modern web applications with React, Next.js, and a backend API",
            prerequisites: &["Node.js 18+", "npm or yarn", "Next.js knowledge"],
        },
        TemplateInfo {
            id: TEMPLATE_RUST_MICROSERVICE,
            name: "Rust Microservice",
            description: "Template for Rust-based microservices and CLI tools",
            use_case: "Building performant backend services, CLI tools, or system utilities in Rust",
            prerequisites: &["Rust 1.70+", "Cargo", "Basic Rust knowledge"],
        },
        TemplateInfo {
            id: TEMPLATE_PYTHON_FASTAPI,
            name: "Python FastAPI",
            description: "Template for Python REST APIs using FastAPI",
            use_case: "Building REST APIs, data processing services, or ML model endpoints",
            prerequisites: &["Python 3.10+", "pip or poetry", "FastAPI knowledge"],
        },
        TemplateInfo {
            id: TEMPLATE_DOCS_REFACTOR,
            name: "Documentation Refactor",
            description: "Template for documentation improvements and refactoring",
            use_case: "Restructuring, improving, or migrating documentation",
            prerequisites: &["Markdown knowledge", "Understanding of target docs"],
        },
    ]
}

/// Get template info by ID
#[must_use]
pub fn get_template(id: &str) -> Option<TemplateInfo> {
    list_templates().into_iter().find(|t| t.id == id)
}

/// Check if a template ID is valid
#[must_use]
pub fn is_valid_template(id: &str) -> bool {
    BUILT_IN_TEMPLATES.contains(&id)
}

/// Initialize a spec from a template
///
/// Creates the spec directory structure and seeds it with template content:
/// - Problem statement in context/
/// - Minimal .xchecker/config.toml (if not exists)
/// - Example partial spec flow
/// - README for the template
///
/// # Arguments
/// * `template_id` - The template identifier
/// * `spec_id` - The spec ID to create
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(_)` if template is invalid or spec creation fails
pub fn init_from_template(template_id: &str, spec_id: &str) -> Result<()> {
    // Validate template
    let template = get_template(template_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown template '{}'. Run 'xchecker template list' to see available templates.",
            template_id
        )
    })?;

    // Create spec directory structure
    let spec_dir = crate::paths::spec_root(spec_id);
    let artifacts_dir = spec_dir.join("artifacts");
    let context_dir = spec_dir.join("context");
    let receipts_dir = spec_dir.join("receipts");

    // Check if spec already exists
    if spec_dir.exists() {
        anyhow::bail!("Spec '{}' already exists at: {}", spec_id, spec_dir);
    }

    // Create directories
    crate::paths::ensure_dir_all(&artifacts_dir)
        .with_context(|| format!("Failed to create artifacts directory: {}", artifacts_dir))?;
    crate::paths::ensure_dir_all(&context_dir)
        .with_context(|| format!("Failed to create context directory: {}", context_dir))?;
    crate::paths::ensure_dir_all(&receipts_dir)
        .with_context(|| format!("Failed to create receipts directory: {}", receipts_dir))?;

    // Generate template content
    let problem_statement = generate_problem_statement(template_id, spec_id);
    let readme_content = generate_readme(&template);

    // Write problem statement to context
    let problem_path = context_dir.join("problem-statement.md");
    write_file_atomic(&problem_path, &problem_statement)
        .with_context(|| format!("Failed to write problem statement: {}", problem_path))?;

    // Write README to spec directory
    let readme_path = spec_dir.join("README.md");
    write_file_atomic(&readme_path, &readme_content)
        .with_context(|| format!("Failed to write README: {}", readme_path))?;

    // Create minimal config if .xchecker/config.toml doesn't exist
    ensure_minimal_config()?;

    Ok(())
}

/// Generate problem statement content for a template
fn generate_problem_statement(template_id: &str, spec_id: &str) -> String {
    match template_id {
        TEMPLATE_FULLSTACK_NEXTJS => format!(
            r#"# Problem Statement: {spec_id}

## Overview

Build a full-stack web application using Next.js with the following capabilities:

- Modern React-based frontend with server-side rendering
- API routes for backend functionality
- Database integration (PostgreSQL/Prisma recommended)
- Authentication and authorization
- Responsive design with Tailwind CSS

## Goals

1. Create a production-ready Next.js application
2. Implement core features with proper error handling
3. Set up testing infrastructure (Jest, React Testing Library)
4. Configure CI/CD pipeline
5. Document API endpoints and usage

## Constraints

- Use TypeScript for type safety
- Follow Next.js App Router conventions
- Implement proper security practices
- Ensure accessibility compliance (WCAG 2.1 AA)

## Success Criteria

- All core features implemented and tested
- Performance targets met (Core Web Vitals)
- Documentation complete
- CI/CD pipeline operational
"#,
            spec_id = spec_id
        ),
        TEMPLATE_RUST_MICROSERVICE => format!(
            r#"# Problem Statement: {spec_id}

## Overview

Build a Rust microservice/CLI tool with the following capabilities:

- High-performance request handling
- Structured logging and observability
- Configuration management
- Graceful shutdown handling
- Comprehensive error handling

## Goals

1. Create a production-ready Rust service
2. Implement core business logic
3. Set up testing infrastructure (unit, integration, property-based)
4. Configure CI/CD pipeline
5. Document API/CLI usage

## Constraints

- Use stable Rust (1.70+)
- Follow Rust idioms and best practices
- Minimize dependencies where practical
- Ensure cross-platform compatibility (Linux, macOS, Windows)

## Success Criteria

- All core features implemented and tested
- Performance targets met
- Documentation complete
- CI/CD pipeline operational
"#,
            spec_id = spec_id
        ),
        TEMPLATE_PYTHON_FASTAPI => format!(
            r#"# Problem Statement: {spec_id}

## Overview

Build a Python REST API using FastAPI with the following capabilities:

- RESTful API endpoints with automatic OpenAPI documentation
- Database integration (SQLAlchemy/PostgreSQL)
- Authentication (JWT/OAuth2)
- Input validation with Pydantic
- Async request handling

## Goals

1. Create a production-ready FastAPI application
2. Implement core API endpoints
3. Set up testing infrastructure (pytest, httpx)
4. Configure CI/CD pipeline
5. Document API endpoints

## Constraints

- Use Python 3.10+
- Follow PEP 8 style guidelines
- Use type hints throughout
- Implement proper error handling

## Success Criteria

- All API endpoints implemented and tested
- OpenAPI documentation complete
- Performance targets met
- CI/CD pipeline operational
"#,
            spec_id = spec_id
        ),
        TEMPLATE_DOCS_REFACTOR => format!(
            r#"# Problem Statement: {spec_id}

## Overview

Refactor and improve documentation with the following goals:

- Restructure documentation for better navigation
- Improve clarity and consistency
- Add missing documentation
- Update outdated content
- Improve code examples

## Goals

1. Audit existing documentation
2. Create documentation structure plan
3. Rewrite/improve key sections
4. Add missing content
5. Validate all code examples

## Constraints

- Maintain backward compatibility with existing links where possible
- Follow documentation style guide
- Ensure all code examples are tested
- Keep documentation in sync with code

## Success Criteria

- Documentation structure improved
- All sections reviewed and updated
- Code examples validated
- Navigation improved
- Search functionality working
"#,
            spec_id = spec_id
        ),
        _ => format!(
            r#"# Problem Statement: {spec_id}

## Overview

[Describe the problem you're trying to solve]

## Goals

1. [Goal 1]
2. [Goal 2]
3. [Goal 3]

## Constraints

- [Constraint 1]
- [Constraint 2]

## Success Criteria

- [Criterion 1]
- [Criterion 2]
"#,
            spec_id = spec_id
        ),
    }
}

/// Generate README content for a template
fn generate_readme(template: &TemplateInfo) -> String {
    let prerequisites = template
        .prerequisites
        .iter()
        .map(|p| format!("- {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# {name}

{description}

## Intended Use

{use_case}

## Prerequisites

{prerequisites}

## Getting Started

1. Review the problem statement in `context/problem-statement.md`
2. Run the requirements phase:
   ```bash
   xchecker resume <spec-id> --phase requirements
   ```
3. Review generated requirements in `artifacts/`
4. Continue with design phase:
   ```bash
   xchecker resume <spec-id> --phase design
   ```
5. Continue through remaining phases as needed

## Basic Flow

```
Requirements → Design → Tasks → Review → Fixup → Final
```

Each phase builds on the previous one:
- **Requirements**: Generate detailed requirements from the problem statement
- **Design**: Create architecture and design documents
- **Tasks**: Break down into implementation tasks
- **Review**: Review and validate the spec
- **Fixup**: Apply any suggested changes
- **Final**: Finalize the spec

## Commands

```bash
# Check spec status
xchecker status <spec-id>

# Resume from a specific phase
xchecker resume <spec-id> --phase <phase>

# Run in dry-run mode (no LLM calls)
xchecker resume <spec-id> --phase <phase> --dry-run
```

## More Information

- [xchecker Documentation](https://github.com/your-org/xchecker)
- [Configuration Guide](../../docs/CONFIGURATION.md)
"#,
        name = template.name,
        description = template.description,
        use_case = template.use_case,
        prerequisites = prerequisites,
    )
}

/// Ensure minimal .xchecker/config.toml exists
fn ensure_minimal_config() -> Result<()> {
    let config_dir = Utf8Path::new(".xchecker");
    let config_path = config_dir.join("config.toml");

    // Only create if it doesn't exist
    if config_path.exists() {
        return Ok(());
    }

    // Create .xchecker directory if needed
    if !config_dir.exists() {
        crate::paths::ensure_dir_all(config_dir)
            .with_context(|| format!("Failed to create config directory: {}", config_dir))?;
    }

    let config_content = r#"# xchecker configuration
# See docs/CONFIGURATION.md for all options

[defaults]
# model = "haiku"
# max_turns = 5

[packet]
# packet_max_bytes = 65536
# packet_max_lines = 1200

[runner]
# runner_mode = "auto"

[llm]
# provider = "claude-cli"
# execution_strategy = "controlled"
"#;

    write_file_atomic(&config_path, config_content)
        .with_context(|| format!("Failed to write config file: {}", config_path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let templates = list_templates();
        assert_eq!(templates.len(), 4);

        let ids: Vec<&str> = templates.iter().map(|t| t.id).collect();
        assert!(ids.contains(&TEMPLATE_FULLSTACK_NEXTJS));
        assert!(ids.contains(&TEMPLATE_RUST_MICROSERVICE));
        assert!(ids.contains(&TEMPLATE_PYTHON_FASTAPI));
        assert!(ids.contains(&TEMPLATE_DOCS_REFACTOR));
    }

    #[test]
    fn test_get_template_valid() {
        let template = get_template(TEMPLATE_FULLSTACK_NEXTJS);
        assert!(template.is_some());
        let t = template.unwrap();
        assert_eq!(t.id, TEMPLATE_FULLSTACK_NEXTJS);
        assert!(!t.name.is_empty());
        assert!(!t.description.is_empty());
    }

    #[test]
    fn test_get_template_invalid() {
        let template = get_template("nonexistent-template");
        assert!(template.is_none());
    }

    #[test]
    fn test_is_valid_template() {
        assert!(is_valid_template(TEMPLATE_FULLSTACK_NEXTJS));
        assert!(is_valid_template(TEMPLATE_RUST_MICROSERVICE));
        assert!(is_valid_template(TEMPLATE_PYTHON_FASTAPI));
        assert!(is_valid_template(TEMPLATE_DOCS_REFACTOR));
        assert!(!is_valid_template("invalid-template"));
    }

    #[test]
    fn test_generate_problem_statement() {
        let content = generate_problem_statement(TEMPLATE_FULLSTACK_NEXTJS, "my-app");
        assert!(content.contains("my-app"));
        assert!(content.contains("Next.js"));
        assert!(content.contains("Problem Statement"));
    }

    #[test]
    fn test_generate_readme() {
        let template = get_template(TEMPLATE_RUST_MICROSERVICE).unwrap();
        let readme = generate_readme(&template);
        assert!(readme.contains("Rust Microservice"));
        assert!(readme.contains("Prerequisites"));
        assert!(readme.contains("Getting Started"));
    }

    #[test]
    fn test_init_from_template() {
        // Use isolated home to avoid conflicts
        let _temp_dir = crate::paths::with_isolated_home();

        let result = init_from_template(TEMPLATE_RUST_MICROSERVICE, "test-rust-service");
        assert!(result.is_ok());

        // Verify directories were created
        let spec_dir = crate::paths::spec_root("test-rust-service");
        assert!(spec_dir.exists());
        assert!(spec_dir.join("artifacts").exists());
        assert!(spec_dir.join("context").exists());
        assert!(spec_dir.join("receipts").exists());

        // Verify files were created
        assert!(spec_dir.join("context/problem-statement.md").exists());
        assert!(spec_dir.join("README.md").exists());
    }

    #[test]
    fn test_init_from_template_invalid() {
        let _temp_dir = crate::paths::with_isolated_home();

        let result = init_from_template("invalid-template", "test-spec");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown template"));
    }

    #[test]
    fn test_init_from_template_already_exists() {
        let _temp_dir = crate::paths::with_isolated_home();

        // Create first spec
        init_from_template(TEMPLATE_PYTHON_FASTAPI, "existing-spec").unwrap();

        // Try to create again
        let result = init_from_template(TEMPLATE_PYTHON_FASTAPI, "existing-spec");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }
}
