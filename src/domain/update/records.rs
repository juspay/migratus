use crate::domain::types::{BatchNumber, LineNumber, PaymentMethodId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Record after loading from CSV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecord {
    pub line_number: LineNumber,
    pub data: HashMap<String, String>,
}

impl UpdateRecord {
    pub fn new(line_number: LineNumber, data: HashMap<String, String>) -> Self {
        Self { line_number, data }
    }
}

/// Successful update result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessfulUpdate {
    pub line_number: LineNumber,
    pub batch_number: BatchNumber,
    pub original_data: HashMap<String, String>,
    pub payment_method_id: PaymentMethodId,
    pub update_status: String,
    pub metadata: UpdateMetadata,
}

/// Failed update result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedUpdate {
    pub line_number: LineNumber,
    pub batch_number: BatchNumber,
    pub original_data: HashMap<String, String>,
    pub failure_reason: UpdateFailureReason,
}

/// Metadata about what was updated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMetadata {
    pub updated_payment_method_data: Option<bool>,
    pub connector_customer_updated: bool,
    pub connector_mandate_details_updated: bool,
}

/// Reason for update failure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateFailureReason {
    RecordLevelFailure(String),
    ApiError { status: u16, message: String },
    ValidationError(String),
}

impl std::fmt::Display for UpdateFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RecordLevelFailure(msg) => write!(f, "Record level failure: {}", msg),
            Self::ApiError { status, message } => write!(f, "API error ({}): {}", status, message),
            Self::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

/// Collection of update results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResults {
    pub successful: Vec<SuccessfulUpdate>,
    pub failed: Vec<FailedUpdate>,
}

impl UpdateResults {
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

impl Default for UpdateResults {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch of update records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBatch {
    pub batch_number: BatchNumber,
    pub records: Vec<UpdateRecord>,
}

impl UpdateBatch {
    pub fn new(batch_number: BatchNumber, records: Vec<UpdateRecord>) -> Self {
        Self {
            batch_number,
            records,
        }
    }
}
