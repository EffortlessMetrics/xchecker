//! Artifact staging, promotion, and receipt hash collection.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::error::{PhaseError, XCheckerError};
use crate::status::artifact::ArtifactType;
use crate::types::{FileHash, FileType, PhaseId};

use super::super::PhaseOrchestrator;

pub(super) struct StoredArtifacts {
    pub(super) paths: Vec<PathBuf>,
    pub(super) output_hashes: Vec<FileHash>,
    pub(super) warnings: Vec<String>,
}

impl PhaseOrchestrator {
    pub(super) fn store_success_artifacts(
        &self,
        phase_id: PhaseId,
        phase_result: &xchecker_phase_api::PhaseResult,
    ) -> Result<StoredArtifacts> {
        let mut output_hashes = Vec::new();
        let mut warnings = Vec::new();

        for artifact in &phase_result.artifacts {
            let partial_result = self
                .artifact_manager()
                .store_partial_staged_artifact(artifact)
                .with_context(|| format!("Failed to store partial artifact: {}", artifact.name))?;

            warnings.extend(
                partial_result
                    .atomic_write_result
                    .warnings
                    .iter()
                    .map(|warning| format!("{}: {}", artifact.name, warning)),
            );

            output_hashes.push(self.create_artifact_hash(phase_id, artifact)?);
        }

        let mut paths = Vec::new();
        for artifact in &phase_result.artifacts {
            let final_path = self
                .artifact_manager()
                .promote_staged_to_final(&artifact.name)
                .with_context(|| {
                    format!("Failed to promote artifact to final: {}", artifact.name)
                })?;
            paths.push(final_path.into_std_path_buf());
        }

        Ok(StoredArtifacts {
            paths,
            output_hashes,
            warnings,
        })
    }

    fn create_artifact_hash(
        &self,
        phase_id: PhaseId,
        artifact: &crate::status::artifact::Artifact,
    ) -> Result<FileHash> {
        let file_type = if let Some(ext) = std::path::Path::new(&artifact.name).extension() {
            FileType::from_extension(ext.to_str().unwrap_or(""))
        } else {
            match artifact.artifact_type {
                ArtifactType::Markdown => FileType::Markdown,
                ArtifactType::CoreYaml => FileType::Yaml,
                _ => FileType::Text,
            }
        };

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
