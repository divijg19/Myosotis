use myosotis::memory::CHECKPOINT_INTERVAL;
use myosotis::node::Value;
use myosotis::{storage, Memory};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}.tmp", path));
}

fn build_state_with_history() -> Result<Memory, Box<dyn std::error::Error>> {
    let mut mem = Memory::new();
    let first = mem.create("Agent");
    mem.set(first, "name", Value::Str("root".to_string()))?;
    mem.commit(Some("c1".to_string()))?;

    for i in 2..=70 {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        if i % 10 == 0 {
            mem.delete_node(id)?;
        }
        mem.commit(Some(format!("c{}", i)))?;
    }
    Ok(mem)
}

#[test]
fn compaction_equivalence() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_equivalence.myo";
    cleanup(path);

    let mem = build_state_with_history()?;
    let before_hash = Memory::compute_state_hash(&mem.head_state);
    storage::save(path, &mem)?;

    storage::compact(path, None)?;
    let after = storage::load(path)?;
    let after_hash = Memory::compute_state_hash(&after.head_state);

    assert_eq!(before_hash, after_hash);
    cleanup(path);
    Ok(())
}

#[test]
fn compaction_at_arbitrary_commit() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_arbitrary.myo";
    cleanup(path);

    let mem = build_state_with_history()?;
    storage::save(path, &mem)?;

    storage::compact(path, Some(25))?;
    let compacted = storage::load(path)?;

    assert!(compacted.genesis_state.is_some());
    assert!(compacted.commits.iter().all(|c| c.id > 25));

    cleanup(path);
    Ok(())
}

#[test]
fn hash_chain_valid_after_compaction() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_hash_chain.myo";
    cleanup(path);

    let mem = build_state_with_history()?;
    storage::save(path, &mem)?;

    storage::compact(path, Some(30))?;
    let compacted = storage::load(path)?;

    compacted.validate()?;

    cleanup(path);
    Ok(())
}

#[test]
fn checkpoint_interaction_after_compaction() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_checkpoint.myo";
    cleanup(path);

    let mut mem = Memory::new();
    for i in 0..(CHECKPOINT_INTERVAL + 5) {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }
    assert!(!mem.checkpoints.is_empty());
    storage::save(path, &mem)?;

    storage::compact(path, None)?;
    let compacted = storage::load(path)?;

    // checkpoints at or before compaction target are removed
    if let Some(first_commit) = compacted.commits.first() {
        assert!(compacted
            .checkpoints
            .iter()
            .all(|cp| cp.commit_id >= first_commit.id));
    }

    cleanup(path);
    Ok(())
}

#[test]
fn tombstone_preservation_after_compaction() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_tombstone.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;
    mem.delete_node(id)?;
    mem.commit(Some("c2".to_string()))?;
    storage::save(path, &mem)?;

    storage::compact(path, Some(1))?;
    let compacted = storage::load(path)?;

    let node = compacted.head_state.get(&id).ok_or("missing node")?;
    assert!(node.deleted);

    cleanup(path);
    Ok(())
}

#[test]
fn cross_restart_stability_after_compaction() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_compaction_restart.myo";
    cleanup(path);

    let mem = build_state_with_history()?;
    storage::save(path, &mem)?;

    storage::compact(path, None)?;
    let loaded = storage::load(path)?;
    storage::save(path, &loaded)?;
    let reloaded = storage::load(path)?;

    assert_eq!(loaded.head_state, reloaded.head_state);

    cleanup(path);
    Ok(())
}
