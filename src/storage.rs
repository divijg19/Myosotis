use crate::memory::Memory;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn save(path: &str, memory: &Memory) -> Result<()> {
    let data = serde_json::to_string_pretty(memory)?;
    fs::write(path, data).with_context(|| format!("Failed to write to file: {}", path))?;
    Ok(())
}

pub fn load(path: &str) -> Result<Memory> {
    let data =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path))?;
    let mem = serde_json::from_str(&data)?;
    Ok(mem)
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}
