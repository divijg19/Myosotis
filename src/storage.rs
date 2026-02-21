use crate::error::MyosotisError;
use crate::memory::Memory;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const FILE_MAGIC: &str = "MYOSOTIS";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub enum LoadMode {
    Strict,
    Unsafe,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageFormatV1 {
    magic: String,
    format_version: u32,
    genesis_state: Option<HashMap<crate::node::NodeId, crate::node::Node>>,
    genesis_state_hash: Option<[u8; 32]>,
    commits: Vec<crate::commit::Commit>,
    checkpoints: Vec<crate::memory::Checkpoint>,
    next_node_id: crate::node::NodeId,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyStorageFormatV05 {
    genesis_state: Option<HashMap<crate::node::NodeId, crate::node::Node>>,
    genesis_state_hash: Option<[u8; 32]>,
    commits: Vec<crate::commit::Commit>,
    checkpoints: Vec<crate::memory::Checkpoint>,
    next_node_id: crate::node::NodeId,
}

fn to_memory(sf: StorageFormatV1) -> Memory {
    let mut mem = Memory::new();
    mem.genesis_state = sf.genesis_state;
    mem.genesis_state_hash = sf.genesis_state_hash;
    mem.commits = sf.commits;
    mem.checkpoints = sf.checkpoints;
    mem.next_node_id = sf.next_node_id;
    mem
}

fn from_memory(memory: &Memory) -> StorageFormatV1 {
    StorageFormatV1 {
        magic: FILE_MAGIC.to_string(),
        format_version: FORMAT_VERSION,
        genesis_state: memory.genesis_state.clone(),
        genesis_state_hash: memory.genesis_state_hash,
        commits: memory.commits.clone(),
        checkpoints: memory.checkpoints.clone(),
        next_node_id: memory.next_node_id,
    }
}

fn validate_and_build_head(mut mem: Memory, mode: LoadMode) -> Result<Memory> {
    let verify_hashes = matches!(mode, LoadMode::Strict);
    mem.validate_with_mode(verify_hashes)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let state = if let Some(cp) = mem.checkpoints.iter().max_by_key(|c| c.commit_id) {
        let start_index = mem
            .commits
            .iter()
            .position(|c| c.id == cp.commit_id)
            .ok_or_else(|| anyhow::anyhow!(MyosotisError::InvalidCheckpoint))?
            + 1;
        Memory::replay_from(cp.state.clone(), &mem.commits[start_index..])
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
    } else {
        Memory::replay_from(mem.genesis_state.clone().unwrap_or_default(), &mem.commits)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
    };

    mem.head_state = state;
    mem.pending_mutations = Vec::new();
    Ok(mem)
}

pub fn save(path: &str, memory: &Memory) -> Result<()> {
    let sf = from_memory(memory);
    let data = serde_json::to_string_pretty(&sf)?;
    fs::write(path, data).with_context(|| format!("Failed to write to file: {}", path))?;
    Ok(())
}

pub fn load_with_mode(path: &str, mode: LoadMode) -> Result<Memory> {
    let data =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))?;

    let root: serde_json::Value =
        serde_json::from_str(&data).map_err(|_| anyhow::anyhow!(MyosotisError::MalformedFileStructure))?;

    let obj = root
        .as_object()
        .ok_or_else(|| anyhow::anyhow!(MyosotisError::MalformedFileStructure))?;

    let has_magic = obj.contains_key("magic");
    let has_version = obj.contains_key("format_version");

    if has_magic && !has_version {
        return Err(anyhow::anyhow!(MyosotisError::MissingFormatVersion));
    }

    if has_version {
        let version = obj
            .get("format_version")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!(MyosotisError::MissingFormatVersion))? as u32;

        if version == 0 {
            return Err(anyhow::anyhow!(MyosotisError::MissingFormatVersion));
        }
        if version > FORMAT_VERSION {
            return Err(anyhow::anyhow!(MyosotisError::UnsupportedFormatVersion(version)));
        }

        let magic = obj
            .get("magic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(MyosotisError::InvalidFileMagic))?;
        if magic != FILE_MAGIC {
            return Err(anyhow::anyhow!(MyosotisError::InvalidFileMagic));
        }

        let sf: StorageFormatV1 = serde_json::from_value(root)
            .map_err(|_| anyhow::anyhow!(MyosotisError::MalformedFileStructure))?;
        let mem = to_memory(sf);
        return validate_and_build_head(mem, mode);
    }

    // Legacy v0.5.0 path: no magic + no format_version
    if has_magic {
        return Err(anyhow::anyhow!(MyosotisError::InvalidFileMagic));
    }

    let legacy: LegacyStorageFormatV05 =
        serde_json::from_str(&data).map_err(|_| anyhow::anyhow!(MyosotisError::MalformedFileStructure))?;
    let sf = StorageFormatV1 {
        magic: FILE_MAGIC.to_string(),
        format_version: FORMAT_VERSION,
        genesis_state: legacy.genesis_state,
        genesis_state_hash: legacy.genesis_state_hash,
        commits: legacy.commits,
        checkpoints: legacy.checkpoints,
        next_node_id: legacy.next_node_id,
    };

    let mem = to_memory(sf);
    validate_and_build_head(mem, mode)
}

pub fn load(path: &str) -> Result<Memory> {
    load_with_mode(path, LoadMode::Strict)
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn compact(path: &str, at: Option<u64>) -> Result<()> {
    let mut mem = load(path)?;
    let before_state_hash = Memory::compute_state_hash(&mem.head_state);

    let target_commit_id = if let Some(target) = at {
        if mem.commits.iter().any(|c| c.id == target) {
            target
        } else {
            return Err(anyhow::anyhow!(MyosotisError::InvalidCompactionTarget));
        }
    } else if let Some(cp) = mem.checkpoints.iter().max_by_key(|c| c.commit_id) {
        cp.commit_id
    } else if let Some(last) = mem.commits.last() {
        last.id
    } else {
        return Err(anyhow::anyhow!(MyosotisError::InvalidCompactionTarget));
    };

    let genesis_state = mem
        .state_at_commit(target_commit_id)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let genesis_state_hash = Memory::compute_state_hash(&genesis_state);

    mem.genesis_state = Some(genesis_state);
    mem.genesis_state_hash = Some(genesis_state_hash);

    mem.commits.retain(|c| c.id > target_commit_id);

    let mut prev_hash = mem.genesis_state_hash;
    let mut prev_id: Option<u64> = None;
    for commit in &mut mem.commits {
        commit.parent = prev_id;
        commit.parent_hash = prev_hash;
        commit.hash = Memory::compute_commit_hash(commit.parent_hash, &commit.message, &commit.mutations);
        prev_hash = Some(commit.hash);
        prev_id = Some(commit.id);
    }

    mem.checkpoints.retain(|cp| cp.commit_id > target_commit_id);
    for checkpoint in &mut mem.checkpoints {
        let commit = mem
            .commits
            .iter()
            .find(|c| c.id == checkpoint.commit_id)
            .ok_or_else(|| anyhow::anyhow!(MyosotisError::CheckpointCommitMismatch))?;
        checkpoint.commit_hash = commit.hash;
    }

    let tmp_path = format!("{}.tmp", path);
    save(&tmp_path, &mem)?;

    let reloaded = load(&tmp_path)?;
    let after_state_hash = Memory::compute_state_hash(&reloaded.head_state);
    if after_state_hash != before_state_hash {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!(MyosotisError::CompactionIntegrityMismatch));
    }

    fs::rename(&tmp_path, path)
        .with_context(|| format!("Failed to atomically replace file: {}", path))?;
    Ok(())
}
