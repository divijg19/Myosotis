use crate::error::MyosotisError;
use crate::memory::Memory;
use anyhow::{Context, Result};
use std::fs;

pub fn compact(path: &str, at: Option<u64>) -> Result<()> {
    let mut mem = crate::storage::load(path)?;
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
    crate::storage::save(&tmp_path, &mem)?;

    let reloaded = crate::storage::load(&tmp_path)?;
    let after_state_hash = Memory::compute_state_hash(&reloaded.head_state);
    if after_state_hash != before_state_hash {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!(MyosotisError::CompactionIntegrityMismatch));
    }

    fs::rename(&tmp_path, path)
        .with_context(|| format!("Failed to atomically replace file: {}", path))?;
    Ok(())
}
