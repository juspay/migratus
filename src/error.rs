use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("File I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Record count mismatch: expected {expected}, got {actual}")]
    RecordCountMismatch { expected: usize, actual: usize },

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Merge error: {0}")]
    MergeError(String),

    #[error("Batch error: {0}")]
    BatchError(String),

    #[error("Migration API error: {0}")]
    ApiError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout error")]
    TimeoutError,

    #[error("Invalid field: {field}, reason: {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Missing required field: {0}")]
    MissingRequiredField(String),

    #[error("Duplicate record: {0}")]
    DuplicateRecord(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, MigrationError>;
