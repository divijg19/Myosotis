use myosotis::commit::Mutation;
use myosotis::node::Value;
use myosotis::{Memory, storage};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn replay_determinism_and_head_equals_replay() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_replay_rt.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;

    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c2".to_string()))?;

    storage::save(path, &mem)?;

    let loaded = storage::load(path)?;

    // head_state equals replay of commits
    let replayed = Memory::replay(&loaded.commits)?;
    assert_eq!(loaded.head_state, replayed);

    // also equal to original mem.head_state
    assert_eq!(mem.head_state, replayed);

    cleanup(path);
    Ok(())
}

#[test]
fn multi_commit_evolution() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_multi_commit.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;

    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c2".to_string()))?;

    storage::save(path, &mem)?;

    let loaded = storage::load(path)?;

    // replay up to first commit: node exists but no 'goal'
    let state1 = Memory::replay(&loaded.commits[..1])?;
    assert!(state1.get(&id).is_some());
    assert!(!state1.get(&id).unwrap().fields.contains_key("goal"));

    // replay up to second commit: has 'goal'
    let state2 = Memory::replay(&loaded.commits[..2])?;
    assert!(state2.get(&id).is_some());
    assert!(state2.get(&id).unwrap().fields.contains_key("goal"));

    cleanup(path);
    Ok(())
}

#[test]
fn invalid_mutation_fails_on_load() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_invalid_mutation.myo";
    cleanup(path);

    // Construct a commit where first mutation is SetField (invalid)
    let mutations = vec![Mutation::SetField {
        id: 1,
        key: "x".to_string(),
        value: Value::Str("v".to_string()),
    }];

    let hash = Memory::compute_commit_hash(None, &Some("bad".to_string()), &mutations);

    let bad_commit = myosotis::commit::Commit {
        id: 1,
        parent: None,
        parent_hash: None,
        hash,
        message: Some("bad".to_string()),
        mutations,
    };

    let mut mem = Memory::new();
    mem.commits.push(bad_commit);
    mem.next_node_id = 2;

    // Save corrupt memory directly via storage (it will serialize commits)
    storage::save(path, &mem)?;

    // Loading should fail validation/replay
    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn corrupt_parent_chain_fails_load() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_corrupt_parent.myo";
    cleanup(path);

    // Create two commits, but make parent of 2 incorrect
    let m1 = vec![Mutation::CreateNode {
        id: 1,
        ty: "Agent".to_string(),
    }];
    let h1 = Memory::compute_commit_hash(None, &Some("c1".to_string()), &m1);
    let c1 = myosotis::commit::Commit {
        id: 1,
        parent: None,
        parent_hash: None,
        hash: h1,
        message: Some("c1".to_string()),
        mutations: m1,
    };

    let m2 = vec![Mutation::SetField {
        id: 1,
        key: "goal".to_string(),
        value: Value::Str("Explore".to_string()),
    }];
    let h2 = Memory::compute_commit_hash(Some(h1), &Some("c2".to_string()), &m2);
    let c2 = myosotis::commit::Commit {
        id: 2,
        parent: Some(999), // invalid
        parent_hash: Some(h1),
        hash: h2,
        message: Some("c2".to_string()),
        mutations: m2,
    };

    let mut mem = Memory::new();
    mem.commits.push(c1);
    mem.commits.push(c2);
    mem.next_node_id = 2;

    storage::save(path, &mem)?;

    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn cross_restart_determinism() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_cross_restart.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;

    storage::save(path, &mem)?;

    // load and save again
    let loaded = storage::load(path)?;
    storage::save(path, &loaded)?;

    let reloaded = storage::load(path)?;

    assert_eq!(loaded.head_state, reloaded.head_state);

    cleanup(path);
    Ok(())
}
