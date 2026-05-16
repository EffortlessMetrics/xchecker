//! Persistent reproducibility locks (`lock.json`, `flow.lock`, promotions).

use camino::Utf8PathBuf;
use chrono::Utc;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::paths::{spec_root, write_file_atomic};
use crate::{
    DriftPair, FlowContext, FlowLock, FlowLockDrift, LockDrift, PromotionArtifact, PromotionLock,
    RunContext, XCheckerLock,
};

impl XCheckerLock {
    /// Create a new lockfile with current context
    #[must_use]
    pub fn new(model_full_name: String, claude_cli_version: String) -> Self {
        Self {
            schema_version: "1".to_string(),
            created_at: Utc::now(),
            model_full_name,
            claude_cli_version,
        }
    }

    /// Detect drift between locked values and current run context
    /// Returns None if no drift detected, Some(LockDrift) if drift exists
    #[must_use]
    pub fn detect_drift(&self, current: &RunContext) -> Option<LockDrift> {
        let mut drift = LockDrift {
            model_full_name: None,
            claude_cli_version: None,
            schema_version: None,
        };

        // Check model drift
        if self.model_full_name != current.model_full_name {
            drift.model_full_name = Some(DriftPair {
                locked: self.model_full_name.clone(),
                current: current.model_full_name.clone(),
            });
        }

        // Check Claude CLI version drift
        if self.claude_cli_version != current.claude_cli_version {
            drift.claude_cli_version = Some(DriftPair {
                locked: self.claude_cli_version.clone(),
                current: current.claude_cli_version.clone(),
            });
        }

        // Check schema version drift
        if self.schema_version != current.schema_version {
            drift.schema_version = Some(DriftPair {
                locked: self.schema_version.clone(),
                current: current.schema_version.clone(),
            });
        }

        // Return None if no drift detected
        if drift.model_full_name.is_none()
            && drift.claude_cli_version.is_none()
            && drift.schema_version.is_none()
        {
            None
        } else {
            Some(drift)
        }
    }

    /// Load lockfile from spec directory
    pub fn load(spec_id: &str) -> Result<Option<Self>, io::Error> {
        let lock_path = Self::get_lock_path(spec_id);

        if !lock_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_path)?;
        let lock: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(lock))
    }

    /// Save lockfile to spec directory
    pub fn save(&self, spec_id: &str) -> Result<(), io::Error> {
        let lock_path = Self::get_lock_path_utf8(spec_id);

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        write_file_atomic(&lock_path, &json).map_err(io::Error::other)?;

        Ok(())
    }

    /// Get the path to the lockfile for a spec ID
    pub(crate) fn get_lock_path(spec_id: &str) -> PathBuf {
        Self::get_lock_path_utf8(spec_id).into_std_path_buf()
    }

    /// Get the UTF-8 path to the lockfile for a spec ID
    pub(crate) fn get_lock_path_utf8(spec_id: &str) -> Utf8PathBuf {
        spec_root(spec_id).join("lock.json")
    }
}

impl FlowLock {
    /// Create a new flow lock from the current governed flow context.
    #[must_use]
    pub fn new(context: FlowContext) -> Self {
        Self {
            schema_version: "1".to_string(),
            created_at: Utc::now(),
            flow_version: context.flow_version,
            stage_graph_version: context.stage_graph_version,
            gate_set_version: context.gate_set_version,
            prompt_pack_version: context.prompt_pack_version,
            adapter_version: context.adapter_version,
            provider_policy: context.provider_policy,
            tool_context_version: context.tool_context_version,
        }
    }

    /// Detect drift between the locked flow boundary and the current flow context.
    #[must_use]
    pub fn detect_drift(&self, current: &FlowContext) -> Option<FlowLockDrift> {
        let mut drift = FlowLockDrift::default();

        if self.flow_version != current.flow_version {
            drift.flow_version = Some(DriftPair {
                locked: self.flow_version.clone(),
                current: current.flow_version.clone(),
            });
        }

        if self.stage_graph_version != current.stage_graph_version {
            drift.stage_graph_version = Some(DriftPair {
                locked: self.stage_graph_version.clone(),
                current: current.stage_graph_version.clone(),
            });
        }

        if self.gate_set_version != current.gate_set_version {
            drift.gate_set_version = Some(DriftPair {
                locked: self.gate_set_version.clone(),
                current: current.gate_set_version.clone(),
            });
        }

        if self.prompt_pack_version != current.prompt_pack_version {
            drift.prompt_pack_version = Some(DriftPair {
                locked: self.prompt_pack_version.clone(),
                current: current.prompt_pack_version.clone(),
            });
        }

        if self.adapter_version != current.adapter_version {
            drift.adapter_version = Some(DriftPair {
                locked: self.adapter_version.clone(),
                current: current.adapter_version.clone(),
            });
        }

        if self.provider_policy != current.provider_policy {
            drift.provider_policy = Some(DriftPair {
                locked: self.provider_policy.clone(),
                current: current.provider_policy.clone(),
            });
        }

        if self.tool_context_version != current.tool_context_version {
            drift.tool_context_version = Some(DriftPair {
                locked: self.tool_context_version.clone(),
                current: current.tool_context_version.clone(),
            });
        }

        if drift.flow_version.is_none()
            && drift.stage_graph_version.is_none()
            && drift.gate_set_version.is_none()
            && drift.prompt_pack_version.is_none()
            && drift.adapter_version.is_none()
            && drift.provider_policy.is_none()
            && drift.tool_context_version.is_none()
        {
            None
        } else {
            Some(drift)
        }
    }

    /// Load `flow.lock` from the spec directory.
    pub fn load(spec_id: &str) -> Result<Option<Self>, io::Error> {
        let lock_path = Self::get_lock_path(spec_id);

        if !lock_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_path)?;
        let lock: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(lock))
    }

    /// Save `flow.lock` to the spec directory.
    pub fn save(&self, spec_id: &str) -> Result<(), io::Error> {
        let lock_path = Self::get_lock_path_utf8(spec_id);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        write_file_atomic(&lock_path, &json).map_err(io::Error::other)?;
        Ok(())
    }

    pub(crate) fn get_lock_path(spec_id: &str) -> PathBuf {
        Self::get_lock_path_utf8(spec_id).into_std_path_buf()
    }

    pub(crate) fn get_lock_path_utf8(spec_id: &str) -> Utf8PathBuf {
        spec_root(spec_id).join("flow.lock")
    }
}

impl PromotionLock {
    /// Create a new promotion lock for a promoted phase result.
    #[must_use]
    pub fn new(
        spec_id: String,
        phase: String,
        gate_name: String,
        gate_version: String,
        artifacts: Vec<PromotionArtifact>,
    ) -> Self {
        Self {
            schema_version: "1".to_string(),
            promoted_at: Utc::now(),
            spec_id,
            phase,
            selected_candidate_id: None,
            gate_name,
            gate_version,
            approved_by: None,
            parent_receipt_path: None,
            parent_packet_lineage: Vec::new(),
            artifacts,
            warnings: Vec::new(),
        }
    }

    /// Load the promotion lock for a specific phase.
    pub fn load(spec_id: &str, phase: &str) -> Result<Option<Self>, io::Error> {
        let lock_path = Self::get_lock_path(spec_id, phase);

        if !lock_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_path)?;
        let lock: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(lock))
    }

    /// List all promotion locks for a spec, sorted by phase name.
    pub fn list(spec_id: &str) -> Result<Vec<Self>, io::Error> {
        let dir = Self::promotions_dir_utf8(spec_id);
        if !dir.as_std_path().exists() {
            return Ok(Vec::new());
        }

        let mut locks = Vec::new();
        for entry in fs::read_dir(dir.as_std_path())? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let lock: Self = serde_json::from_str(&content)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            locks.push(lock);
        }

        locks.sort_by(|a, b| a.phase.cmp(&b.phase));
        Ok(locks)
    }

    /// Save this promotion lock under `promotions/<phase>.lock`.
    pub fn save(&self) -> Result<(), io::Error> {
        let lock_path = Self::get_lock_path_utf8(&self.spec_id, &self.phase);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        write_file_atomic(&lock_path, &json).map_err(io::Error::other)?;
        Ok(())
    }

    pub(crate) fn get_lock_path(spec_id: &str, phase: &str) -> PathBuf {
        Self::get_lock_path_utf8(spec_id, phase).into_std_path_buf()
    }

    pub(crate) fn get_lock_path_utf8(spec_id: &str, phase: &str) -> Utf8PathBuf {
        Self::promotions_dir_utf8(spec_id).join(format!("{}.lock", sanitize_lock_component(phase)))
    }

    pub(crate) fn promotions_dir_utf8(spec_id: &str) -> Utf8PathBuf {
        spec_root(spec_id).join("promotions")
    }
}

fn sanitize_lock_component(input: &str) -> String {
    let sanitized: String = input
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect();

    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}
