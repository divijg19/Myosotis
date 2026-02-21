use myosotis::Memory;
use myosotis::node::Value;
use myosotis::storage::{self, FILE_MAGIC, FORMAT_VERSION, LoadMode};
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}.tmp", path));
}

#[test]
fn header_validation_test() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_header_validation.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;
    storage::save(path, &mem)?;

    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;

    // Missing magic should fail in v1 schema
    if let Some(obj) = json.as_object_mut() {
        obj.remove("magic");
    }
    fs::write(path, serde_json::to_string_pretty(&json)?)?;
    assert!(storage::load(path).is_err());

    // Wrong magic should fail
    let mut json2: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if let Some(obj) = json2.as_object_mut() {
        obj.insert(
            "magic".to_string(),
            serde_json::Value::String("WRONG".to_string()),
        );
        obj.insert(
            "format_version".to_string(),
            serde_json::Value::Number(serde_json::Number::from(FORMAT_VERSION)),
        );
    }
    fs::write(path, serde_json::to_string_pretty(&json2)?)?;
    assert!(storage::load(path).is_err());

    cleanup(path);
    Ok(())
}

#[test]
fn format_version_test_and_legacy_migration_path() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_format_version.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;
    storage::save(path, &mem)?;

    // too-new version should fail
    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if let Some(obj) = json.as_object_mut() {
        obj.insert("format_version".to_string(), serde_json::json!(2));
    }
    fs::write(path, serde_json::to_string_pretty(&json)?)?;
    assert!(storage::load(path).is_err());

    // remove both fields => legacy migration path should load
    let mut legacy_json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if let Some(obj) = legacy_json.as_object_mut() {
        obj.remove("magic");
        obj.remove("format_version");
    }
    fs::write(path, serde_json::to_string_pretty(&legacy_json)?)?;
    let loaded = storage::load(path)?;
    storage::save(path, &loaded)?;

    let post: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let obj = post.as_object().ok_or("not object")?;
    assert_eq!(obj.get("magic").and_then(|v| v.as_str()), Some(FILE_MAGIC));
    assert_eq!(
        obj.get("format_version").and_then(|v| v.as_u64()),
        Some(FORMAT_VERSION as u64)
    );

    cleanup(path);
    Ok(())
}

#[test]
fn migration_preserves_hash_equivalence() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_migration_hash_equivalence.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;
    storage::save(path, &mem)?;

    let before_hash = Memory::compute_state_hash(&mem.head_state);

    let mut legacy_json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if let Some(obj) = legacy_json.as_object_mut() {
        obj.remove("magic");
        obj.remove("format_version");
    }
    fs::write(path, serde_json::to_string_pretty(&legacy_json)?)?;

    let loaded = storage::load(path)?;
    let migrated_hash = Memory::compute_state_hash(&loaded.head_state);
    assert_eq!(before_hash, migrated_hash);

    cleanup(path);
    Ok(())
}

#[test]
fn strict_vs_unsafe_mode() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_strict_unsafe.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.set(id, "goal", Value::Str("Explore".to_string()))?;
    mem.commit(Some("c1".to_string()))?;
    storage::save(path, &mem)?;

    // Tamper commit hash
    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    if let Some(commits) = json.get_mut("commits").and_then(|v| v.as_array_mut()) {
        if let Some(first) = commits.first_mut() {
            first["hash"] = serde_json::json!(vec![0u8; 32]);
        }
    }
    fs::write(path, serde_json::to_string_pretty(&json)?)?;

    assert!(storage::load_with_mode(path, LoadMode::Strict).is_err());
    assert!(storage::load_with_mode(path, LoadMode::Unsafe).is_ok());

    cleanup(path);
    Ok(())
}

#[test]
fn corrupt_genesis_detection() -> Result<(), Box<dyn std::error::Error>> {
    let path = "test_corrupt_genesis.myo";
    cleanup(path);

    let mut mem = Memory::new();
    let id = mem.create("Agent");
    mem.commit(Some("c1".to_string()))?;
    mem.delete_node(id)?;
    mem.commit(Some("c2".to_string()))?;

    storage::save(path, &mem)?;
    storage::compact(path, Some(1))?;

    let mut json: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    json["genesis_state_hash"] = serde_json::json!(vec![1u8; 32]);
    fs::write(path, serde_json::to_string_pretty(&json)?)?;

    assert!(storage::load(path).is_err());

    cleanup(path);
    Ok(())
}
