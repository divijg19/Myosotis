use crate::error::MyosotisError;
use crate::memory::Memory;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct StorageFormat {
    genesis_state: Option<std::collections::HashMap<crate::node::NodeId, crate::node::Node>>,
    genesis_state_hash: Option<[u8; 32]>,
    commits: Vec<crate::commit::Commit>,
    checkpoints: Vec<crate::memory::Checkpoint>,
    next_node_id: crate::node::NodeId,
}

pub fn save(path: &str, memory: &Memory) -> Result<()> {
    let sf = StorageFormat {
        genesis_state: memory.genesis_state.clone(),
        genesis_state_hash: memory.genesis_state_hash,
        commits: memory.commits.clone(),
        checkpoints: memory.checkpoints.clone(),
        next_node_id: memory.next_node_id,
    };

    let data = serde_json::to_string_pretty(&sf)?;
    fs::write(path, data).with_context(|| format!("Failed to write to file: {}", path))?;
    Ok(())
}

pub fn load(path: &str) -> Result<Memory> {
    let data =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))?;
    let sf: StorageFormat = serde_json::from_str(&data)?;

    // Build memory from storage format and replay to construct head_state
    let mut mem = Memory::new();
    mem.genesis_state = sf.genesis_state;
    mem.genesis_state_hash = sf.genesis_state_hash;
    mem.commits = sf.commits;
    mem.checkpoints = sf.checkpoints;
    mem.next_node_id = sf.next_node_id;

    // Validate commit chain + checkpoint integrity
    mem.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Reconstruct head_state from latest valid checkpoint then replay remaining commits.
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
        commit.hash =
            Memory::compute_commit_hash(commit.parent_hash, &commit.message, &commit.mutations);
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
