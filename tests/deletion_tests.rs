use myosotis::memory::CHECKPOINT_INTERVAL;
use myosotis::node::Value;
use myosotis::{Memory, MyosotisError, storage};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn node_deletion_replay() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("create".to_string()))?;

    mem.delete_node(id)?;
    mem.commit(Some("delete".to_string()))?;

    let replayed = Memory::replay(&mem.commits)?;
    let node = replayed.get(&id).ok_or("missing node")?;
    assert!(node.deleted);
    Ok(())
}

#[test]
fn field_deletion_replay() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.delete_field(id, "goal")?;
    mem.commit(Some("field-delete".to_string()))?;

    let replayed = Memory::replay(&mem.commits)?;
    let node = replayed.get(&id).ok_or("missing node")?;
    assert!(!node.fields.contains_key("goal"));
    Ok(())
}

#[test]
fn double_delete_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("create".to_string()))?;

    mem.delete_node(id)?;
    mem.commit(Some("delete".to_string()))?;

    let err = mem.delete_node(id).expect_err("second delete should fail");
    assert!(matches!(err, MyosotisError::DeleteOnDeletedNode(_)));
    Ok(())
}

#[test]
fn delete_after_checkpoint() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_delete_after_checkpoint.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let first = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;

    for i in 1..CHECKPOINT_INTERVAL {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }

    assert!(!mem.checkpoints.is_empty());

    mem.delete_node(first)?;
    mem.commit(Some("delete-after-cp".to_string()))?;

    storage::save(path, &mem)?;
    let loaded = storage::load(path)?;

    let node = loaded.head_state.get(&first).ok_or("missing node")?;
    assert!(node.deleted);

    cleanup(path);
    Ok(())
}

#[test]
fn historical_query_before_after_delete() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("before-delete".to_string()))?;

    mem.delete_node(id)?;
    mem.commit(Some("after-delete".to_string()))?;

    let before = mem.state_at_commit(1)?;
    let before_node = before.get(&id).ok_or("missing before node")?;
    assert!(!before_node.deleted);

    let after = mem.state_at_commit(2)?;
    let after_node = after.get(&id).ok_or("missing after node")?;
    assert!(after_node.deleted);

    Ok(())
}

#[test]
fn hash_stability_after_deletion() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_hash_stability_delete.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;

    let before_hash = Memory::compute_state_hash(&mem.head_state);

    mem.delete_node(id)?;
    mem.commit(Some("c2".to_string()))?;

    let after_hash = Memory::compute_state_hash(&mem.head_state);
    assert_ne!(before_hash, after_hash);

    storage::save(path, &mem)?;
    let loaded = storage::load(path)?;
    let loaded_hash = Memory::compute_state_hash(&loaded.head_state);
    assert_eq!(after_hash, loaded_hash);

    cleanup(path);
    Ok(())
}
