use myosotis::node::Value;
use myosotis::{Memory, storage};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn hash_chain_validation() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_hash_chain.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;

    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c2".to_string()))?;

    // Validate stored hashes equal recomputed ones
    for (i, commit) in mem.commits.iter().enumerate() {
        let parent_hash = if i == 0 {
            None
        } else {
            Some(mem.commits[i - 1].hash)
        };
        let recomputed =
            Memory::compute_commit_hash(parent_hash, &commit.message, &commit.mutations);
        assert_eq!(commit.hash, recomputed);
    }

    storage::save(path, &mem)?;
    cleanup(path);
    Ok(())
}

#[test]
fn parent_hash_corruption_detected() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_parent_corrupt.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c2".to_string()))?;

    storage::save(path, &mem)?;

    // Tamper with parent_hash of second commit in the saved JSON
    let mut data: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if let Some(commits) = data.get_mut("commits").and_then(|c| c.as_array_mut()) {
        if commits.len() >= 2 {
            if let Some(obj) = commits[1].as_object_mut() {
                obj.insert("parent_hash".to_string(), serde_json::Value::Null);
            }
        }
    }

    std::fs::write(path, serde_json::to_string_pretty(&data)?)?;

    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn mutation_tampering_detected() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_mutation_tamper.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;

    storage::save(path, &mem)?;

    // Tamper with commit message (which is part of hash input)
    let mut json: String = std::fs::read_to_string(path)?;
    json = json.replace("c1", "tampered");
    std::fs::write(path, json)?;

    let res = storage::load(path);
    assert!(res.is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn cross_restart_hash_stability() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_cross_hash.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;

    storage::save(path, &mem)?;

    let loaded = storage::load(path)?;

    for (i, commit) in loaded.commits.iter().enumerate() {
        let parent_hash = if i == 0 {
            None
        } else {
            Some(loaded.commits[i - 1].hash)
        };
        let recomputed =
            Memory::compute_commit_hash(parent_hash, &commit.message, &commit.mutations);
        assert_eq!(commit.hash, recomputed);
    }

    cleanup(path);
    Ok(())
}
