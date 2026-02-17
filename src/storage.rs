use crate::memory::Memory;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct StorageFormat {
    commits: Vec<crate::commit::Commit>,
    checkpoints: Vec<crate::memory::Checkpoint>,
    next_node_id: crate::node::NodeId,
}

pub fn save(path: &str, memory: &Memory) -> Result<()> {
    let sf = StorageFormat {
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
    mem.commits = sf.commits;
    mem.checkpoints = sf.checkpoints;
    mem.next_node_id = sf.next_node_id;

    // Validate commit chain + checkpoint integrity
    mem.validate().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Reconstruct head_state from latest valid checkpoint then replay remaining commits.
    let state = if let Some(cp) = mem.checkpoints.iter().max_by_key(|c| c.commit_id) {
        let start_index = cp.commit_id as usize;
        Memory::replay_from(cp.state.clone(), &mem.commits[start_index..])
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
    } else {
        Memory::replay(&mem.commits).map_err(|e| anyhow::anyhow!(e.to_string()))?
    };
    mem.head_state = state;
    mem.pending_mutations = Vec::new();

    Ok(mem)
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}
