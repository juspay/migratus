use crate::domain::types::{BatchNumber, LineNumber, MerchantId, PaymentMethodId};
use crate::error::{MigrationError, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::PathBuf;

pub const PAYMENT_METHOD_FINGERPRINT_ID_FIELDS: [PaymentMethodFingerprintIdField; 2] = [
    PaymentMethodFingerprintIdField::MerchantId,
    PaymentMethodFingerprintIdField::PaymentMethodId,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdLoadedRecord {
    pub line_number: LineNumber,
    pub fields: Vec<String>,
    pub original_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdMigrationRecord {
    pub line_number: LineNumber,
    pub merchant_id: MerchantId,
    pub payment_method_id: PaymentMethodId,
    pub original_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidPaymentMethodFingerprintIdRecord {
    pub line_number: LineNumber,
    pub original_data: HashMap<String, String>,
    pub invalid_reason: PaymentMethodFingerprintIdInvalidReason,
    pub failed_at_stage: PaymentMethodFingerprintIdInvalidStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedPaymentMethodFingerprintIdRecord {
    pub line_number: LineNumber,
    pub merchant_id: MerchantId,
    pub payment_method_id: PaymentMethodId,
    pub original_data: HashMap<String, String>,
    pub skip_reason: PaymentMethodFingerprintIdSkipReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdBatch {
    pub batch_number: BatchNumber,
    pub file_name: String,
    pub records: Vec<PaymentMethodFingerprintIdMigrationRecord>,
    pub byte_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPaymentMethodFingerprintIdBatchResponse {
    pub batch_number: usize,
    pub batch_file: String,
    pub record_count: usize,
    pub byte_size: usize,
    pub endpoint: String,
    pub started_at: String,
    pub completed_at: String,
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
    pub transport_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdApiResponse {
    #[serde(default)]
    pub total_rows: usize,
    #[serde(default)]
    pub successful_count: usize,
    #[serde(default)]
    pub failed_count: usize,
    #[serde(default)]
    pub results: Vec<PaymentMethodFingerprintIdApiRowResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdApiRowResult {
    pub row_number: Option<i64>,
    pub merchant_id: Option<MerchantId>,
    pub payment_method_id: Option<PaymentMethodId>,
    #[serde(default)]
    pub old_fingerprint_id: Option<String>,
    #[serde(default)]
    pub new_fingerprint_id: Option<String>,
    pub migration_status: PaymentMethodFingerprintIdStatus,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl PaymentMethodFingerprintIdApiRowResult {
    pub fn failure_reason(&self) -> Option<String> {
        self.error
            .clone()
            .or_else(|| self.message.clone())
            .or_else(|| self.reason.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdJsonlResult {
    pub batch_number: usize,
    pub batch_file: String,
    pub row_number: Option<i64>,
    pub merchant_id: Option<MerchantId>,
    pub payment_method_id: Option<PaymentMethodId>,
    pub old_fingerprint_id: Option<String>,
    pub new_fingerprint_id: Option<String>,
    pub migration_status: PaymentMethodFingerprintIdStatus,
    pub error: Option<String>,
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodFingerprintIdMigrationSummary {
    pub total_input_rows: usize,
    pub valid_rows: usize,
    pub enriched_rows: usize,
    pub invalid_input_rows: usize,
    pub already_migrated_rows: usize,
    pub total_batches: usize,
    pub api_total_rows: usize,
    pub successful_count: usize,
    pub failed_count: usize,
    pub transport_error_count: usize,
    pub per_status_counts: BTreeMap<PaymentMethodFingerprintIdStatus, usize>,
}

#[derive(Debug, Clone)]
pub struct PaymentMethodFingerprintIdBatchFile {
    pub batch_number: usize,
    pub path: PathBuf,
    pub record_count: usize,
    pub byte_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct PaymentMethodFingerprintIdHeaderIndex {
    pub merchant_id: usize,
    pub payment_method_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodFingerprintIdField {
    MerchantId,
    PaymentMethodId,
}

impl PaymentMethodFingerprintIdField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MerchantId => "merchant_id",
            Self::PaymentMethodId => "payment_method_id",
        }
    }
}

impl fmt::Display for PaymentMethodFingerprintIdField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethodFingerprintIdInvalidStage {
    Validation,
    Batching,
}

impl fmt::Display for PaymentMethodFingerprintIdInvalidStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation => write!(f, "Validation"),
            Self::Batching => write!(f, "Batching"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethodFingerprintIdInvalidReason {
    MissingRequiredField {
        field: PaymentMethodFingerprintIdField,
    },
    DuplicateRecord {
        first_line_number: usize,
    },
    RowExceedsMaxFileSize {
        actual: usize,
        max: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethodFingerprintIdSkipReason {
    AlreadyMigrated,
}

impl fmt::Display for PaymentMethodFingerprintIdSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyMigrated => write!(f, "Already migrated"),
        }
    }
}

impl fmt::Display for PaymentMethodFingerprintIdInvalidReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredField { field } => {
                write!(f, "Missing required field: {}", field)
            }
            Self::DuplicateRecord { first_line_number } => {
                write!(
                    f,
                    "Duplicate merchant_id/payment_method_id pair first seen at line {}",
                    first_line_number
                )
            }
            Self::RowExceedsMaxFileSize { actual, max } => {
                write!(
                    f,
                    "Single row exceeds max_file_size_bytes: {} > {}",
                    actual, max
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaymentMethodFingerprintIdStatus {
    Success,
    TransportError,
    Unknown(String),
}

impl PaymentMethodFingerprintIdStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Success => "Success",
            Self::TransportError => "transport_error",
            Self::Unknown(status) => status.as_str(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

impl fmt::Display for PaymentMethodFingerprintIdStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for PaymentMethodFingerprintIdStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PaymentMethodFingerprintIdStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let status = String::deserialize(deserializer)?;
        Ok(match status.as_str() {
            "Success" => Self::Success,
            "transport_error" => Self::TransportError,
            _ => Self::Unknown(status),
        })
    }
}

pub fn row_to_original_data(
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
) -> HashMap<String, String> {
    let mut data = HashMap::new();
    for (index, value) in record.iter().enumerate() {
        let key = headers
            .get(index)
            .map(String::from)
            .unwrap_or_else(|| format!("extra_field_{}", index + 1));
        data.insert(key, value.to_string());
    }
    data
}

pub fn loaded_record_from_csv(
    headers: &csv::StringRecord,
    header_index: PaymentMethodFingerprintIdHeaderIndex,
    line_number: usize,
    record: csv::StringRecord,
) -> PaymentMethodFingerprintIdLoadedRecord {
    PaymentMethodFingerprintIdLoadedRecord {
        line_number: LineNumber::new(line_number),
        fields: vec![
            record
                .get(header_index.merchant_id)
                .unwrap_or_default()
                .to_string(),
            record
                .get(header_index.payment_method_id)
                .unwrap_or_default()
                .to_string(),
        ],
        original_data: row_to_original_data(headers, &record),
    }
}

pub fn validate_headers(
    headers: &csv::StringRecord,
) -> Result<PaymentMethodFingerprintIdHeaderIndex> {
    let merchant_id = find_required_header(headers, PaymentMethodFingerprintIdField::MerchantId)?;
    let payment_method_id =
        find_required_header(headers, PaymentMethodFingerprintIdField::PaymentMethodId)?;

    Ok(PaymentMethodFingerprintIdHeaderIndex {
        merchant_id,
        payment_method_id,
    })
}

fn find_required_header(
    headers: &csv::StringRecord,
    field: PaymentMethodFingerprintIdField,
) -> Result<usize> {
    headers
        .iter()
        .position(|header| header == field.as_str())
        .ok_or_else(|| {
            MigrationError::ValidationError(format!(
                "Payment method fingerprint ID CSV must include required header: {}",
                field
            ))
        })
}

pub fn validate_loaded_record(
    record: PaymentMethodFingerprintIdLoadedRecord,
) -> std::result::Result<
    PaymentMethodFingerprintIdMigrationRecord,
    InvalidPaymentMethodFingerprintIdRecord,
> {
    let merchant_id = record
        .fields
        .first()
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let payment_method_id = record
        .fields
        .get(1)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    if merchant_id.is_empty() {
        return Err(InvalidPaymentMethodFingerprintIdRecord {
            line_number: record.line_number,
            original_data: record.original_data,
            invalid_reason: PaymentMethodFingerprintIdInvalidReason::MissingRequiredField {
                field: PaymentMethodFingerprintIdField::MerchantId,
            },
            failed_at_stage: PaymentMethodFingerprintIdInvalidStage::Validation,
        });
    }

    if payment_method_id.is_empty() {
        return Err(InvalidPaymentMethodFingerprintIdRecord {
            line_number: record.line_number,
            original_data: record.original_data,
            invalid_reason: PaymentMethodFingerprintIdInvalidReason::MissingRequiredField {
                field: PaymentMethodFingerprintIdField::PaymentMethodId,
            },
            failed_at_stage: PaymentMethodFingerprintIdInvalidStage::Validation,
        });
    }

    Ok(PaymentMethodFingerprintIdMigrationRecord {
        line_number: record.line_number,
        merchant_id: MerchantId::new(merchant_id),
        payment_method_id: PaymentMethodId::new(payment_method_id),
        original_data: record.original_data,
    })
}

pub fn records_to_csv_bytes(
    records: &[PaymentMethodFingerprintIdMigrationRecord],
) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record(
        PAYMENT_METHOD_FINGERPRINT_ID_FIELDS
            .iter()
            .map(PaymentMethodFingerprintIdField::as_str),
    )?;

    for record in records {
        writer.write_record([record.merchant_id.inner(), record.payment_method_id.inner()])?;
    }

    writer
        .into_inner()
        .map_err(|e| MigrationError::InternalError(format!("CSV writer error: {}", e)))
}

pub fn split_records_into_batches(
    records: Vec<PaymentMethodFingerprintIdMigrationRecord>,
    batch_size: usize,
    max_file_size_bytes: usize,
) -> Result<(
    Vec<PaymentMethodFingerprintIdBatch>,
    Vec<InvalidPaymentMethodFingerprintIdRecord>,
)> {
    if batch_size == 0 {
        return Err(MigrationError::BatchError(
            "batch_size must be greater than zero".to_string(),
        ));
    }

    if max_file_size_bytes == 0 {
        return Err(MigrationError::BatchError(
            "max_file_size_bytes must be greater than zero".to_string(),
        ));
    }

    let mut batches = Vec::new();
    let mut invalid = Vec::new();
    let mut current = Vec::new();

    for record in records {
        let single_record_bytes = records_to_csv_bytes(std::slice::from_ref(&record))?;
        if single_record_bytes.len() > max_file_size_bytes {
            invalid.push(InvalidPaymentMethodFingerprintIdRecord {
                line_number: record.line_number,
                original_data: record.original_data,
                invalid_reason: PaymentMethodFingerprintIdInvalidReason::RowExceedsMaxFileSize {
                    actual: single_record_bytes.len(),
                    max: max_file_size_bytes,
                },
                failed_at_stage: PaymentMethodFingerprintIdInvalidStage::Batching,
            });
            continue;
        }

        let mut candidate = current.clone();
        candidate.push(record.clone());
        let candidate_bytes = records_to_csv_bytes(&candidate)?;

        if !current.is_empty()
            && (candidate.len() > batch_size || candidate_bytes.len() > max_file_size_bytes)
        {
            push_batch(&mut batches, std::mem::take(&mut current))?;
            current.push(record);
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        push_batch(&mut batches, current)?;
    }

    Ok((batches, invalid))
}

fn push_batch(
    batches: &mut Vec<PaymentMethodFingerprintIdBatch>,
    records: Vec<PaymentMethodFingerprintIdMigrationRecord>,
) -> Result<()> {
    let batch_number = batches.len() + 1;
    let byte_size = records_to_csv_bytes(&records)?.len();
    batches.push(PaymentMethodFingerprintIdBatch {
        batch_number: BatchNumber::new(batch_number),
        file_name: format!("batch_{:04}.csv", batch_number),
        records,
        byte_size,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        line: usize,
        merchant_id: &str,
        payment_method_id: &str,
    ) -> PaymentMethodFingerprintIdMigrationRecord {
        let mut original_data = HashMap::new();
        original_data.insert("merchant_id".to_string(), merchant_id.to_string());
        original_data.insert(
            "payment_method_id".to_string(),
            payment_method_id.to_string(),
        );
        PaymentMethodFingerprintIdMigrationRecord {
            line_number: LineNumber::new(line),
            merchant_id: MerchantId::new(merchant_id.to_string()),
            payment_method_id: PaymentMethodId::new(payment_method_id.to_string()),
            original_data,
        }
    }

    #[test]
    fn validates_required_headers_and_allows_extra_headers() {
        let headers = csv::StringRecord::from(vec!["merchant_id", "payment_method_id"]);
        let header_index = validate_headers(&headers).unwrap();
        assert_eq!(header_index.merchant_id, 0);
        assert_eq!(header_index.payment_method_id, 1);

        let headers = csv::StringRecord::from(vec![
            "version",
            "merchant_id",
            "id",
            "payment_method_id",
            "extra",
        ]);
        let header_index = validate_headers(&headers).unwrap();
        assert_eq!(header_index.merchant_id, 1);
        assert_eq!(header_index.payment_method_id, 3);
    }

    #[test]
    fn rejects_missing_required_fields() {
        let headers = csv::StringRecord::from(vec!["merchant_id", "payment_method_id"]);
        let header_index = validate_headers(&headers).unwrap();

        let loaded = loaded_record_from_csv(
            &headers,
            header_index,
            2,
            csv::StringRecord::from(vec!["", "pm_123"]),
        );
        let invalid = validate_loaded_record(loaded).unwrap_err();
        assert!(matches!(
            invalid.invalid_reason,
            PaymentMethodFingerprintIdInvalidReason::MissingRequiredField {
                field: PaymentMethodFingerprintIdField::MerchantId
            }
        ));
    }

    #[test]
    fn splits_records_by_count_and_names_batches() {
        let records = vec![
            record(2, "merchant_1", "pm_1"),
            record(3, "merchant_1", "pm_2"),
            record(4, "merchant_1", "pm_3"),
        ];

        let (batches, invalid) = split_records_into_batches(records, 2, 1024).unwrap();
        assert!(invalid.is_empty());
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].file_name, "batch_0001.csv");
        assert_eq!(batches[0].records.len(), 2);
        assert_eq!(batches[1].file_name, "batch_0002.csv");
        assert_eq!(batches[1].records.len(), 1);
    }

    #[test]
    fn serializes_expected_csv_headers() {
        let bytes = records_to_csv_bytes(&[record(2, "merchant_1", "pm_1")]).unwrap();
        let csv = String::from_utf8(bytes).unwrap();
        assert!(csv.starts_with("merchant_id,payment_method_id"));
        assert!(csv.contains("merchant_1,pm_1"));
    }

    #[test]
    fn parses_success_and_unknown_statuses() {
        let success: PaymentMethodFingerprintIdStatus =
            serde_json::from_str("\"Success\"").unwrap();
        assert_eq!(success, PaymentMethodFingerprintIdStatus::Success);

        let failed: PaymentMethodFingerprintIdStatus = serde_json::from_str("\"Failed\"").unwrap();
        assert_eq!(
            failed,
            PaymentMethodFingerprintIdStatus::Unknown("Failed".to_string())
        );
        assert!(!failed.is_success());
    }
}
