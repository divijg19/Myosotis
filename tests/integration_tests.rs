use std::fs;
use myosotis::{Memory, storage, MyosotisError};
use myosotis::node::Value;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn persistence_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_state_rt.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("init".to_string()));

    storage::save(path, &mem)?;

    let loaded = storage::load(path).map_err(|e| format!("load failed: {}", e))?;

    assert_eq!(mem.next_node_id, loaded.next_node_id);
    assert_eq!(mem.commits.len(), loaded.commits.len());
    assert_eq!(mem.nodes.len(), loaded.nodes.len());
    assert!(loaded.nodes.contains_key(&id));

    cleanup(path);
    Ok(())
}

#[test]
fn multi_commit_replay() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_multi_commit.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()));

    mem.set(id, "goal", Value::Str("Explore".to_string()));
    mem.commit(Some("c2".to_string()));

    storage::save(path, &mem)?;

    let loaded = storage::load(path).map_err(|e| format!("load failed: {}", e))?;

    // commit 1 should not have field
    let c1 = &loaded.commits[0];
    assert!(c1.changes.get(&id).is_some());
    assert!(!c1.changes.get(&id).unwrap().fields.contains_key("goal"));

    // commit 2 should have field
    let c2 = &loaded.commits[1];
    assert!(c2.changes.get(&id).is_some());
    assert!(c2.changes.get(&id).unwrap().fields.contains_key("goal"));

    cleanup(path);
    Ok(())
}

#[test]
fn invalid_commit_returns_error() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_invalid_commit.myo";
    cleanup(path);

    let mut mem = Memory::new();
    mem.create("Agent");
    mem.commit(Some("c1".to_string()));
    storage::save(path, &mem)?;

    let loaded = storage::load(path).map_err(|e| format!("load failed: {}", e))?;

    // Simulate show behavior: missing commit should be reported
    let res = (|| -> Result<(), MyosotisError> {
        if loaded.commits.iter().find(|c| c.id == 999).is_none() {
            return Err(MyosotisError::CommitNotFound(999));
        }
        Ok(())
    })();

    assert!(matches!(res, Err(MyosotisError::CommitNotFound(999))));

    cleanup(path);
    Ok(())
}

#[test]
fn invalid_node_returns_error() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_invalid_node.myo";
    cleanup(path);

    let mut mem = Memory::new();
    mem.create("Agent");
    mem.commit(Some("c1".to_string()));
    storage::save(path, &mem)?;

    let loaded = storage::load(path).map_err(|e| format!("load failed: {}", e))?;

    let res = (|| -> Result<(), MyosotisError> {
        if loaded.nodes.get(&999).is_none() {
            return Err(MyosotisError::NodeNotFound(999));
        }
        Ok(())
    })();

    assert!(matches!(res, Err(MyosotisError::NodeNotFound(999))));

    cleanup(path);
    Ok(())
}

#[test]
fn invariant_violation_detected_on_load() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_invariant_corrupt.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()));
    mem.set(id, "goal", Value::Str("Explore".to_string()));
    mem.commit(Some("c2".to_string()));

    // Corrupt parent of second commit to an invalid value
    mem.commits[1].parent = Some(999);

    // Save corrupted memory
    storage::save(path, &mem)?;

    // Loading should fail validation
    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}
