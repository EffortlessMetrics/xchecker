use anyhow::Result;

/// Count pending fixups for a spec.
pub(crate) fn count_pending_fixups_for_spec(spec_id: &str) -> u32 {
    crate::fixup::pending_fixups_for_spec(spec_id).targets
}

/// Check for pending fixups and display intended targets (R5.6).
pub(super) fn check_and_display_fixup_targets(spec_id: &str) -> Result<()> {
    use crate::fixup::{FixupMode, FixupParser};

    // Check if Review phase is completed and has fixup markers
    let base_path = crate::paths::spec_root(spec_id);
    let review_md_path = base_path.join("artifacts").join("30-review.md");

    if !review_md_path.exists() {
        return Ok(()); // No review phase completed yet
    }

    // Read the review content
    let review_content = match std::fs::read_to_string(&review_md_path) {
        Ok(content) => content,
        Err(_) => return Ok(()), // Can't read review file, skip fixup check
    };

    // Create fixup parser in preview mode to check for targets
    let fixup_parser = FixupParser::new(FixupMode::Preview, base_path.clone().into())?;

    // Check if there are fixup markers
    if !fixup_parser.has_fixup_markers(&review_content) {
        return Ok(()); // No fixups needed
    }

    // Parse diffs to get intended targets
    match fixup_parser.parse_diffs(&review_content) {
        Ok(diffs) => {
            if !diffs.is_empty() {
                println!("\n  Pending fixups detected:");
                println!("    Fixup markers found in review phase");
                println!("    Intended targets ({} files):", diffs.len());

                for diff in &diffs {
                    println!("      - {}", diff.target_file);
                }

                // Show preview information
                match fixup_parser.preview_changes(&diffs) {
                    Ok(preview) => {
                        if !preview.all_valid {
                            println!("    ⚠ Warning: Some diffs failed validation");
                        }

                        if !preview.warnings.is_empty() {
                            println!("    Validation warnings:");
                            for warning in &preview.warnings {
                                println!("      - {warning}");
                            }
                        }

                        // Show change summary
                        let mut total_added = 0;
                        let mut total_removed = 0;
                        for (file, summary) in &preview.change_summary {
                            total_added += summary.lines_added;
                            total_removed += summary.lines_removed;
                            if !summary.validation_passed {
                                println!("      ✗ {file}: validation failed");
                            }
                        }

                        if total_added > 0 || total_removed > 0 {
                            println!(
                                "    Estimated changes: +{total_added} lines, -{total_removed} lines"
                            );
                        }
                    }
                    Err(e) => {
                        println!("    ⚠ Warning: Failed to preview changes: {e}");
                    }
                }

                println!("\n    To apply fixups:");
                println!("      xchecker resume {spec_id} --phase fixup --apply-fixups");
                println!("    To preview only (default):");
                println!("      xchecker resume {spec_id} --phase fixup");
            }
        }
        Err(e) => {
            println!("\n  Fixup parsing error: {e}");
            println!("    Review phase contains fixup markers but diffs could not be parsed");
        }
    }
    Ok(())
}
