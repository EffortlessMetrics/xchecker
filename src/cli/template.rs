//! Template CLI subcommands and handlers.
//!
//! This module owns template command parsing, validation, initialization, and
//! user-facing output.

use anyhow::Result;
use clap::Subcommand;

use crate::XCheckerError;
use crate::error::ConfigError;
use crate::spec_id::sanitize_spec_id;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available built-in templates
    ///
    /// Shows all available templates with their descriptions and use cases.
    ///
    /// EXAMPLES:
    ///   xchecker template list
    ///
    /// Per FR-TEMPLATES (Requirements 4.7.1)
    List,

    /// Initialize a spec from a template
    ///
    /// Creates a new spec with predefined problem statement, configuration,
    /// and example partial spec flow based on the selected template.
    ///
    /// Available templates:
    ///   - fullstack-nextjs: Full-stack web applications with Next.js
    ///   - rust-microservice: Rust microservices and CLI tools
    ///   - python-fastapi: Python REST APIs with FastAPI
    ///   - docs-refactor: Documentation improvements and refactoring
    ///
    /// EXAMPLES:
    ///   xchecker template init fullstack-nextjs my-app
    ///   xchecker template init rust-microservice my-service
    ///   xchecker template init python-fastapi my-api
    ///   xchecker template init docs-refactor docs-v2
    ///
    /// Per FR-TEMPLATES (Requirements 4.7.2, 4.7.3)
    Init {
        /// Template to use (fullstack-nextjs, rust-microservice, python-fastapi, docs-refactor)
        template: String,

        /// Spec ID to create
        spec_id: String,
    },
}

pub(super) fn execute_template_command(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::List => {
            println!("Available templates:\n");

            for t in xchecker_engine::templates::list_templates() {
                println!("  {}", t.id);
                println!("    Name: {}", t.name);
                println!("    Description: {}", t.description);
                println!("    Use case: {}", t.use_case);
                if !t.prerequisites.is_empty() {
                    println!("    Prerequisites: {}", t.prerequisites.join(", "));
                }
                println!();
            }

            println!("To initialize a spec from a template:");
            println!("  xchecker template init <template> <spec-id>");

            Ok(())
        }
        TemplateCommands::Init { template, spec_id } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;

            // Validate template
            if !xchecker_engine::templates::is_valid_template(&template) {
                let valid_templates = xchecker_engine::templates::BUILT_IN_TEMPLATES.join(", ");
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "template".to_string(),
                    value: format!(
                        "Unknown template '{}'. Valid templates: {}",
                        template, valid_templates
                    ),
                })
                .into());
            }

            // Initialize from template
            xchecker_engine::templates::init_from_template(&template, &sanitized_id)?;

            // Get template info for display
            let template_info = xchecker_engine::templates::get_template(&template).unwrap();

            println!(
                "✓ Initialized spec '{}' from template '{}'",
                sanitized_id, template
            );
            println!();
            println!("Template: {}", template_info.name);
            println!("Description: {}", template_info.description);
            println!();
            println!("Created files:");
            println!(
                "  - .xchecker/specs/{}/context/problem-statement.md",
                sanitized_id
            );
            println!("  - .xchecker/specs/{}/README.md", sanitized_id);
            println!();
            println!("Next steps:");
            println!("  1. Review the problem statement:");
            println!(
                "     cat .xchecker/specs/{}/context/problem-statement.md",
                sanitized_id
            );
            println!("  2. Customize the problem statement for your needs");
            println!("  3. Run the requirements phase:");
            println!("     xchecker resume {} --phase requirements", sanitized_id);

            Ok(())
        }
    }
}
