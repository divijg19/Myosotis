use myosotis::memory::CHECKPOINT_INTERVAL;
use myosotis::node::Value;
use myosotis::{Memory, storage};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn checkpoint_creation_test() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();

    for i in 0..CHECKPOINT_INTERVAL {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }

    assert_eq!(mem.checkpoints.len(), 1);
    assert_eq!(mem.checkpoints[0].commit_id as usize, CHECKPOINT_INTERVAL);
    Ok(())
}

#[test]
fn replay_equivalence_test() -> Result<(), Box<dyn std::error::Error>> {
    let mut mem = Memory::new();

    for i in 0..(CHECKPOINT_INTERVAL + 5) {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }

    let full = Memory::replay(&mem.commits)?;
    let cp = mem
        .checkpoints
        .iter()
        .max_by_key(|c| c.commit_id)
        .ok_or("missing checkpoint")?;
    let start_index = cp.commit_id as usize;
    let from_cp = Memory::replay_from(cp.state.clone(), &mem.commits[start_index..])?;

    assert_eq!(full, from_cp);
    Ok(())
}

#[test]
fn checkpoint_integrity_test() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_checkpoint_integrity.myo";
    cleanup(path);

    let mut mem = Memory::new();
    for i in 0..CHECKPOINT_INTERVAL {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }
    storage::save(path, &mem)?;

    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let checkpoints = json
        .get_mut("checkpoints")
        .and_then(|v| v.as_array_mut())
        .ok_or("missing checkpoints")?;
    let state = checkpoints[0]
        .get_mut("state")
        .and_then(|v| v.as_object_mut())
        .ok_or("missing checkpoint state")?;

    if let Some((_k, node_val)) = state.iter_mut().next() {
        if let Some(node_obj) = node_val.as_object_mut() {
            node_obj.insert(
                "ty".to_string(),
                serde_json::Value::String("Tampered".to_string()),
            );
        }
    }

    fs::write(path, serde_json::to_string_pretty(&json)?)?;
    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn checkpoint_commit_hash_mismatch_test() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_checkpoint_commit_hash_mismatch.myo";
    cleanup(path);

    let mut mem = Memory::new();
    for i in 0..CHECKPOINT_INTERVAL {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }
    storage::save(path, &mem)?;

    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let checkpoints = json
        .get_mut("checkpoints")
        .and_then(|v| v.as_array_mut())
        .ok_or("missing checkpoints")?;

    checkpoints[0]["commit_hash"] = serde_json::json!(vec![0u8; 32]);

    fs::write(path, serde_json::to_string_pretty(&json)?)?;
    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn cross_restart_determinism_with_checkpoint_test() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_cross_restart_checkpoint.myo";
    cleanup(path);

    let mut mem = Memory::new();
    for i in 0..(CHECKPOINT_INTERVAL + 3) {
        let id = mem.create("Agent");
        mem.set(id, "n", Value::Int(i as i64))?;
        mem.commit(Some(format!("c{}", i + 1)))?;
    }

    storage::save(path, &mem)?;

    let loaded = storage::load(path)?;
    storage::save(path, &loaded)?;
    let reloaded = storage::load(path)?;

    assert_eq!(loaded.head_state, reloaded.head_state);
    assert_eq!(loaded.commits.len(), reloaded.commits.len());

    cleanup(path);
    Ok(())
}
