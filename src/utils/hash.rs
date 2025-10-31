use sha2::{Digest, Sha256};
use std::path::Path;

/// Calculate SHA256 hash of a config file
pub fn calculate_config_hash(config_path: &Path) -> crate::error::Result<String> {
    let config_content = std::fs::read_to_string(config_path)?;
    let mut hasher = Sha256::new();
    hasher.update(config_content.as_bytes());
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Verify that a JSON file's config hash matches the current config
pub fn verify_config_hash(
    json_content: &str,
    config_path: &Path,
) -> crate::error::Result<bool> {
    let current_hash = calculate_config_hash(config_path)?;
    
    // Extract hash from JSON
    let json: serde_json::Value = serde_json::from_str(json_content)?;
    
    if let Some(stored_hash) = json.get("config_hash").and_then(|h| h.as_str()) {
        Ok(stored_hash == current_hash)
    } else {
        // No hash found in JSON - might be old format
        Ok(false)
    }
}
