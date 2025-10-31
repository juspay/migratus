use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub line_number: LineNumber,
    pub data: HashMap<String, String>,
}

impl Record {
    pub fn new(line_number: LineNumber, data: HashMap<String, String>) -> Self {
        Self { line_number, data }
    }

    pub fn get(&self, field: &super::migration_field::MigrationField) -> Option<&String> {
        self.data.get(&field.to_header_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidRecord {
    pub line_number: LineNumber,
    pub original_data: HashMap<String, String>,
    pub invalid_reason: InvalidReason,
    pub failed_at_state: StateName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidReason {
    MissingRequiredField(String),
    InvalidFormat { field: String, message: String },
    DuplicateRecord(String),
    MergeFailure(String),
    ValidationFailure(String),
}

impl std::fmt::Display for InvalidReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequiredField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidFormat { field, message } => {
                write!(f, "Invalid format for {}: {}", field, message)
            }
            Self::DuplicateRecord(msg) => write!(f, "Duplicate record: {}", msg),
            Self::MergeFailure(msg) => write!(f, "Merge failure: {}", msg),
            Self::ValidationFailure(msg) => write!(f, "Validation failure: {}", msg),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StateName {
    Merge,
    Validation,
    Enrichment,
    Batching,
    Migration,
}

impl StateName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Merge => "Merge",
            Self::Validation => "Validation",
            Self::Enrichment => "Enrichment",
            Self::Batching => "Batching",
            Self::Migration => "Migration",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedRecord {
    pub line_number: LineNumber,
    pub data: HashMap<String, String>,
}

impl MergedRecord {
    pub fn new(line_number: LineNumber, data: HashMap<String, String>) -> Self {
        Self { line_number, data }
    }

    pub fn from_record(record: Record) -> Self {
        Self {
            line_number: record.line_number,
            data: record.data,
        }
    }

    /// Get field value using MigrationField enum
    pub fn get_field(&self, field: &super::migration_field::MigrationField) -> Option<&String> {
        self.data.get(&field.to_header_name())
    }

    /// Check if field exists and is non-empty
    pub fn has_field(&self, field: &super::migration_field::MigrationField) -> bool {
        self.get_field(field)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedRecord {
    pub line_number: LineNumber,
    pub data: HashMap<String, String>,
}

impl EnrichedRecord {
    pub fn new(line_number: LineNumber, data: HashMap<String, String>) -> Self {
        Self { line_number, data }
    }

    pub fn from_merged(record: MergedRecord) -> Self {
        Self {
            line_number: record.line_number,
            data: record.data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub batch_number: BatchNumber,
    pub records: Vec<EnrichedRecord>,
}

impl Batch {
    pub fn new(batch_number: BatchNumber, records: Vec<EnrichedRecord>) -> Self {
        Self {
            batch_number,
            records,
        }
    }

    pub fn size(&self) -> usize {
        self.records.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessfulMigration {
    pub line_number: LineNumber,
    pub batch_number: BatchNumber,
    pub original_data: HashMap<String, String>,
    pub payment_method_id: PaymentMethodId,
    pub customer_id: CustomerId,
    pub migration_status: String,
    pub metadata: MigrationMetadata,
    pub payment_method: Option<String>,
    pub payment_method_type: Option<String>,
    pub card_number_masked: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MigrationMetadata {
    pub card_migrated: Option<bool>,
    pub network_token_migrated: bool,
    pub connector_mandate_details_migrated: bool,
    pub network_transaction_id_migrated: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedMigration {
    pub line_number: LineNumber,
    pub batch_number: BatchNumber,
    pub original_data: HashMap<String, String>,
    pub failure_reason: MigrationFailureReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationFailureReason {
    BatchLevelFailure(String),
    RecordLevelFailure(String),
    ApiError { status: u16, message: String },
    NetworkError(String),
    TimeoutError,
}

impl std::fmt::Display for MigrationFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BatchLevelFailure(msg) => write!(f, "Batch failure: {}", msg),
            Self::RecordLevelFailure(msg) => write!(f, "Record failure: {}", msg),
            Self::ApiError { status, message } => {
                write!(f, "API error ({}): {}", status, message)
            }
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::TimeoutError => write!(f, "Request timeout"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResults {
    pub successful: Vec<SuccessfulMigration>,
    pub failed: Vec<FailedMigration>,
}

impl MigrationResults {
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn total(&self) -> usize {
        self.successful.len() + self.failed.len()
    }
}

impl Default for MigrationResults {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSummary {
    pub total_input_records: usize,
    pub valid_for_migration: usize,
    pub invalid_pre_migration: usize,
    pub successful_migrations: usize,
    pub failed_migrations: usize,
    pub total_output_records: usize,
    pub invalid_at_merge: usize,
    pub invalid_at_validation: usize,
}

impl MigrationSummary {
    pub fn verify_invariant(&self) -> Result<(), String> {
        if self.total_input_records != self.total_output_records {
            return Err(format!(
                "Record count mismatch: input={}, output={}",
                self.total_input_records, self.total_output_records
            ));
        }
        Ok(())
    }
}
