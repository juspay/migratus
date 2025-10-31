use serde::{Deserialize, Serialize};

/// Wrapper for intermediate pipeline outputs with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntermediateOutput<T> {
    /// SHA256 hash of the config file used
    pub config_hash: String,
    /// Timestamp when this output was created
    pub timestamp: String,
    /// Number of records in this output
    pub record_count: usize,
    /// The actual records
    pub records: Vec<T>,
}

impl<T> IntermediateOutput<T> {
    pub fn new(config_hash: String, records: Vec<T>) -> Self {
        let record_count = records.len();
        let timestamp = chrono::Utc::now().to_rfc3339();
        
        Self {
            config_hash,
            timestamp,
            record_count,
            records,
        }
    }

    pub fn from_records(config_hash: String, records: Vec<T>) -> Self {
        Self::new(config_hash, records)
    }

    pub fn into_records(self) -> Vec<T> {
        self.records
    }

    pub fn verify_hash(&self, expected_hash: &str) -> bool {
        self.config_hash == expected_hash
    }
}
