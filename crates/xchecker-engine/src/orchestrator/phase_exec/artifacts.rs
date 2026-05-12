//! Artifact staging, promotion, and receipt hash preparation.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::error::{PhaseError, XCheckerError};
use crate::status::artifact::{Artifact, ArtifactType};
use crate::types::{FileHash, FileType, PhaseId};

use crate::orchestrator::PhaseOrchestrator;

pub(crate) struct StoredArtifacts {
    pub artifact_paths: Vec<PathBuf>,
    pub output_hashes: Vec<FileHash>,
    pub atomic_write_warnings: Vec<String>,
}

impl PhaseOrchestrator {
    /// Stage, hash, and promote all successful phase artifacts.
    pub(crate) fn store_success_artifacts(
        &self,
        phase_id: PhaseId,
        artifacts: &[Artifact],
    ) -> Result<StoredArtifacts> {
        let mut output_hashes = Vec::new();
        let mut atomic_write_warnings = Vec::new();

        for artifact in artifacts {
            let partial_result = self
                .artifact_manager()
                .store_partial_staged_artifact(artifact)
                .with_context(|| format!("Failed to store partial artifact: {}", artifact.name))?;

            for warning in &partial_result.atomic_write_result.warnings {
                atomic_write_warnings.push(format!("{}: {}", artifact.name, warning));
            }

            output_hashes.push(self.create_receipt_file_hash(phase_id, artifact)?);
        }

        let mut artifact_paths = Vec::new();
        for artifact in artifacts {
            let final_path = self
                .artifact_manager()
                .promote_staged_to_final(&artifact.name)
                .with_context(|| {
                    format!("Failed to promote artifact to final: {}", artifact.name)
                })?;

            artifact_paths.push(final_path.into_std_path_buf());
        }

        Ok(StoredArtifacts {
            artifact_paths,
            output_hashes,
            atomic_write_warnings,
        })
    }

    /// Stage and promote artifacts when the caller does not need receipt hashes.
    pub(crate) fn store_core_phase_artifacts(&self, artifacts: &[Artifact]) -> Result<()> {
        for artifact in artifacts {
            self.artifact_manager()
                .store_partial_staged_artifact(artifact)
                .with_context(|| format!("Failed to store partial artifact: {}", artifact.name))?;
        }

        for artifact in artifacts {
            self.artifact_manager()
                .promote_staged_to_final(&artifact.name)
                .with_context(|| {
                    format!("Failed to promote artifact to final: {}", artifact.name)
                })?;
        }

        Ok(())
    }

    /// Persist failed LLM output as the phase partial artifact required by R4.3.
    pub(crate) fn store_failed_llm_partial(
        &self,
        phase_id: PhaseId,
        claude_response: &str,
    ) -> Result<(String, PathBuf)> {
        let partial_filename = format!(
            "{:02}-{}.partial.md",
            self.get_phase_number(phase_id),
            phase_id.as_str().to_lowercase()
        );

        let partial_result = self.artifact_manager().store_artifact(&Artifact {
            name: partial_filename.clone(),
            content: claude_response.to_string(),
            artifact_type: ArtifactType::Partial,
            blake3_hash: blake3::hash(claude_response.as_bytes())
                .to_hex()
                .to_string(),
        })?;

        Ok((partial_filename, partial_result.path.into_std_path_buf()))
    }

    fn create_receipt_file_hash(&self, phase_id: PhaseId, artifact: &Artifact) -> Result<FileHash> {
        let file_type = artifact_file_type(artifact);
        self.receipt_manager()
            .create_file_hash(
                &format!("artifacts/{}", artifact.name),
                &artifact.content,
                file_type,
                phase_id.as_str(),
            )
            .map_err(|e| {
                XCheckerError::Phase(PhaseError::OutputValidationFailed {
                    phase: phase_id.as_str().to_string(),
                    reason: e.to_string(),
                })
                .into()
            })
    }
}

fn artifact_file_type(artifact: &Artifact) -> FileType {
    if let Some(ext) = Path::new(&artifact.name).extension() {
        return FileType::from_extension(ext.to_str().unwrap_or(""));
    }

    match artifact.artifact_type {
        ArtifactType::Markdown => FileType::Markdown,
        ArtifactType::CoreYaml => FileType::Yaml,
        _ => FileType::Text,
    }
}
