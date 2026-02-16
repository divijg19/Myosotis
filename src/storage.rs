use crate::memory::Memory;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct StorageFormat {
    commits: Vec<crate::commit::Commit>,
    next_node_id: crate::node::NodeId,
}

pub fn save(path: &str, memory: &Memory) -> Result<()> {
    let sf = StorageFormat {
        commits: memory.commits.clone(),
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
    mem.commits = sf.commits;
    mem.next_node_id = sf.next_node_id;

    // Validate commit chain basic invariants before replay
    mem.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Verify hash chain and recompute hashes
    let mut prev_hash: Option<[u8; 32]> = None;
    for commit in &mem.commits {
        // parent id was validated; check parent_hash matches prev_hash
        if commit.parent_hash != prev_hash {
            return Err(anyhow::anyhow!(
                crate::error::MyosotisError::ParentHashMismatch(commit.id)
            ));
        }

        let recomputed = Memory::compute_commit_hash(prev_hash, &commit.message, &commit.mutations);
        if commit.hash != recomputed {
            return Err(anyhow::anyhow!(crate::error::MyosotisError::InvalidHash));
        }

        prev_hash = Some(commit.hash);
    }

    // Reconstruct head_state via replay
    let state = Memory::replay(&mem.commits).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    mem.head_state = state;
    mem.pending_mutations = Vec::new();

    Ok(mem)
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}
