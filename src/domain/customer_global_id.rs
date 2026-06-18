use crate::domain::types::{BatchNumber, CustomerId, LineNumber, MerchantId};
use crate::error::{MigrationError, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::PathBuf;

pub const CUSTOMER_GLOBAL_ID_FIELDS: [CustomerGlobalIdField; 2] = [
    CustomerGlobalIdField::MerchantId,
    CustomerGlobalIdField::CustomerId,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdLoadedRecord {
    pub line_number: LineNumber,
    pub fields: Vec<String>,
    pub original_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdMigrationRecord {
    pub line_number: LineNumber,
    pub merchant_id: MerchantId,
    pub customer_id: CustomerId,
    pub original_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidCustomerGlobalIdRecord {
    pub line_number: LineNumber,
    pub original_data: HashMap<String, String>,
    pub invalid_reason: CustomerGlobalIdInvalidReason,
    pub failed_at_stage: CustomerGlobalIdInvalidStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdBatch {
    pub batch_number: BatchNumber,
    pub file_name: String,
    pub records: Vec<CustomerGlobalIdMigrationRecord>,
    pub byte_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCustomerGlobalIdBatchResponse {
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
pub struct CustomerGlobalIdApiResponse {
    #[serde(default)]
    pub updated_count: usize,
    #[serde(default)]
    pub skipped_count: usize,
    #[serde(default)]
    pub failed_count: usize,
    #[serde(default)]
    pub results: Vec<CustomerGlobalIdApiRowResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdApiRowResult {
    #[serde(alias = "row_number")]
    pub line_number: Option<i64>,
    pub merchant_id: Option<MerchantId>,
    pub customer_id: Option<CustomerId>,
    pub status: CustomerGlobalIdStatus,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdJsonlResult {
    pub batch_number: usize,
    pub batch_file: String,
    pub merchant_id: Option<MerchantId>,
    pub customer_id: Option<CustomerId>,
    pub status: CustomerGlobalIdStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdMigrationSummary {
    pub total_input_rows: usize,
    pub valid_rows: usize,
    pub invalid_input_rows: usize,
    pub total_batches: usize,
    pub total_updated_count: usize,
    pub total_skipped_count: usize,
    pub total_failed_count: usize,
    pub per_status_counts: BTreeMap<CustomerGlobalIdStatus, usize>,
}

#[derive(Debug, Clone)]
pub struct CustomerGlobalIdBatchFile {
    pub batch_number: usize,
    pub path: PathBuf,
    pub record_count: usize,
    pub byte_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct CustomerGlobalIdHeaderIndex {
    pub merchant_id: usize,
    pub customer_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerGlobalIdField {
    MerchantId,
    CustomerId,
}

impl CustomerGlobalIdField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MerchantId => "merchant_id",
            Self::CustomerId => "customer_id",
        }
    }
}

impl fmt::Display for CustomerGlobalIdField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomerGlobalIdInvalidStage {
    Validation,
    Batching,
}

impl fmt::Display for CustomerGlobalIdInvalidStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation => write!(f, "Validation"),
            Self::Batching => write!(f, "Batching"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomerGlobalIdInvalidReason {
    MissingRequiredField { field: CustomerGlobalIdField },
    RowExceedsMaxFileSize { actual: usize, max: usize },
}

impl fmt::Display for CustomerGlobalIdInvalidReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredField { field } => {
                write!(f, "Missing required field: {}", field)
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
pub enum CustomerGlobalIdStatus {
    UpdatedNullId,
    UpdatedNonGlobalId,
    AlreadyGlobalId,
    SkippedNonV1,
    NotFound,
    InvalidCsvRow,
    UpdateFailed,
    TransportError,
    Unknown(String),
}

impl CustomerGlobalIdStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::UpdatedNullId => "updated_null_id",
            Self::UpdatedNonGlobalId => "updated_non_global_id",
            Self::AlreadyGlobalId => "already_global_id",
            Self::SkippedNonV1 => "skipped_non_v1",
            Self::NotFound => "not_found",
            Self::InvalidCsvRow => "invalid_csv_row",
            Self::UpdateFailed => "update_failed",
            Self::TransportError => "transport_error",
            Self::Unknown(status) => status.as_str(),
        }
    }

    pub fn known_api_statuses() -> [Self; 7] {
        [
            Self::UpdatedNullId,
            Self::UpdatedNonGlobalId,
            Self::AlreadyGlobalId,
            Self::SkippedNonV1,
            Self::NotFound,
            Self::InvalidCsvRow,
            Self::UpdateFailed,
        ]
    }

    pub fn is_updated(&self) -> bool {
        matches!(self, Self::UpdatedNullId | Self::UpdatedNonGlobalId)
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            Self::NotFound | Self::InvalidCsvRow | Self::UpdateFailed | Self::TransportError
        )
    }
}

impl fmt::Display for CustomerGlobalIdStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for CustomerGlobalIdStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CustomerGlobalIdStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let status = String::deserialize(deserializer)?;
        Ok(match status.as_str() {
            "updated_null_id" => Self::UpdatedNullId,
            "updated_non_global_id" => Self::UpdatedNonGlobalId,
            "already_global_id" => Self::AlreadyGlobalId,
            "skipped_non_v1" => Self::SkippedNonV1,
            "not_found" => Self::NotFound,
            "invalid_csv_row" => Self::InvalidCsvRow,
            "update_failed" => Self::UpdateFailed,
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
    header_index: CustomerGlobalIdHeaderIndex,
    line_number: usize,
    record: csv::StringRecord,
) -> CustomerGlobalIdLoadedRecord {
    CustomerGlobalIdLoadedRecord {
        line_number: LineNumber::new(line_number),
        fields: vec![
            record
                .get(header_index.merchant_id)
                .unwrap_or_default()
                .to_string(),
            record
                .get(header_index.customer_id)
                .unwrap_or_default()
                .to_string(),
        ],
        original_data: row_to_original_data(headers, &record),
    }
}

pub fn validate_headers(headers: &csv::StringRecord) -> Result<CustomerGlobalIdHeaderIndex> {
    let merchant_id = find_required_header(headers, CustomerGlobalIdField::MerchantId)?;
    let customer_id = find_required_header(headers, CustomerGlobalIdField::CustomerId)?;

    Ok(CustomerGlobalIdHeaderIndex {
        merchant_id,
        customer_id,
    })
}

fn find_required_header(
    headers: &csv::StringRecord,
    field: CustomerGlobalIdField,
) -> Result<usize> {
    headers
        .iter()
        .position(|header| header == field.as_str())
        .ok_or_else(|| {
            MigrationError::ValidationError(format!(
                "Customer global ID CSV must include required header: {}",
                field
            ))
        })
}

pub fn validate_record(
    line_number: usize,
    record: csv::StringRecord,
) -> std::result::Result<CustomerGlobalIdMigrationRecord, InvalidCustomerGlobalIdRecord> {
    let headers = csv::StringRecord::from(vec!["merchant_id", "customer_id"]);
    let header_index = CustomerGlobalIdHeaderIndex {
        merchant_id: 0,
        customer_id: 1,
    };
    validate_loaded_record(loaded_record_from_csv(
        &headers,
        header_index,
        line_number,
        record,
    ))
}

pub fn validate_loaded_record(
    record: CustomerGlobalIdLoadedRecord,
) -> std::result::Result<CustomerGlobalIdMigrationRecord, InvalidCustomerGlobalIdRecord> {
    let merchant_id = record
        .fields
        .first()
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let customer_id = record
        .fields
        .get(1)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    if merchant_id.is_empty() {
        return Err(InvalidCustomerGlobalIdRecord {
            line_number: record.line_number,
            original_data: record.original_data,
            invalid_reason: CustomerGlobalIdInvalidReason::MissingRequiredField {
                field: CustomerGlobalIdField::MerchantId,
            },
            failed_at_stage: CustomerGlobalIdInvalidStage::Validation,
        });
    }

    if customer_id.is_empty() {
        return Err(InvalidCustomerGlobalIdRecord {
            line_number: record.line_number,
            original_data: record.original_data,
            invalid_reason: CustomerGlobalIdInvalidReason::MissingRequiredField {
                field: CustomerGlobalIdField::CustomerId,
            },
            failed_at_stage: CustomerGlobalIdInvalidStage::Validation,
        });
    }

    Ok(CustomerGlobalIdMigrationRecord {
        line_number: record.line_number,
        merchant_id: MerchantId::new(merchant_id),
        customer_id: CustomerId::new(customer_id),
        original_data: record.original_data,
    })
}

pub fn records_to_csv_bytes(records: &[CustomerGlobalIdMigrationRecord]) -> Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record(
        CUSTOMER_GLOBAL_ID_FIELDS
            .iter()
            .map(CustomerGlobalIdField::as_str),
    )?;

    for record in records {
        writer.write_record([record.merchant_id.inner(), record.customer_id.inner()])?;
    }

    writer
        .into_inner()
        .map_err(|e| MigrationError::InternalError(format!("CSV writer error: {}", e)))
}

pub fn split_records_into_batches(
    records: Vec<CustomerGlobalIdMigrationRecord>,
    batch_size: usize,
    max_file_size_bytes: usize,
) -> Result<(
    Vec<CustomerGlobalIdBatch>,
    Vec<InvalidCustomerGlobalIdRecord>,
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
            invalid.push(InvalidCustomerGlobalIdRecord {
                line_number: record.line_number,
                original_data: record.original_data,
                invalid_reason: CustomerGlobalIdInvalidReason::RowExceedsMaxFileSize {
                    actual: single_record_bytes.len(),
                    max: max_file_size_bytes,
                },
                failed_at_stage: CustomerGlobalIdInvalidStage::Batching,
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
    batches: &mut Vec<CustomerGlobalIdBatch>,
    records: Vec<CustomerGlobalIdMigrationRecord>,
) -> Result<()> {
    let batch_number = batches.len() + 1;
    let byte_size = records_to_csv_bytes(&records)?.len();
    batches.push(CustomerGlobalIdBatch {
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
        customer_id: &str,
    ) -> CustomerGlobalIdMigrationRecord {
        let mut original_data = HashMap::new();
        original_data.insert("merchant_id".to_string(), merchant_id.to_string());
        original_data.insert("customer_id".to_string(), customer_id.to_string());
        CustomerGlobalIdMigrationRecord {
            line_number: LineNumber::new(line),
            merchant_id: MerchantId::new(merchant_id.to_string()),
            customer_id: CustomerId::new(customer_id.to_string()),
            original_data,
        }
    }

    #[test]
    fn validates_required_headers_and_allows_extra_headers() {
        let headers = csv::StringRecord::from(vec!["merchant_id", "customer_id"]);
        let header_index = validate_headers(&headers).unwrap();
        assert_eq!(header_index.merchant_id, 0);
        assert_eq!(header_index.customer_id, 1);

        let headers = csv::StringRecord::from(vec![
            "version",
            "merchant_id",
            "id",
            "customer_id",
            "invalid_case",
        ]);
        let header_index = validate_headers(&headers).unwrap();
        assert_eq!(header_index.merchant_id, 1);
        assert_eq!(header_index.customer_id, 3);

        let headers = csv::StringRecord::from(vec!["merchant_id", "version"]);
        assert!(validate_headers(&headers).is_err());
    }

    #[test]
    fn validates_required_fields_and_field_count() {
        assert!(validate_record(2, csv::StringRecord::from(vec!["m1", "c1"])).is_ok());

        let missing_merchant = validate_record(2, csv::StringRecord::from(vec!["", "c1"]))
            .expect_err("empty merchant_id should fail");
        assert_eq!(
            missing_merchant.invalid_reason,
            CustomerGlobalIdInvalidReason::MissingRequiredField {
                field: CustomerGlobalIdField::MerchantId,
            }
        );

        let missing_customer = validate_record(3, csv::StringRecord::from(vec!["m1", ""]))
            .expect_err("empty customer_id should fail");
        assert_eq!(
            missing_customer.invalid_reason,
            CustomerGlobalIdInvalidReason::MissingRequiredField {
                field: CustomerGlobalIdField::CustomerId,
            }
        );

        let headers = csv::StringRecord::from(vec![
            "version",
            "merchant_id",
            "id",
            "customer_id",
            "invalid_case",
        ]);
        let header_index = validate_headers(&headers).unwrap();
        let loaded = loaded_record_from_csv(
            &headers,
            header_index,
            4,
            csv::StringRecord::from(vec!["v1", "m1", "row-id", "c1", "anything"]),
        );
        let valid = validate_loaded_record(loaded).expect("extra fields should be ignored");
        assert_eq!(valid.merchant_id.inner(), "m1");
        assert_eq!(valid.customer_id.inner(), "c1");
        assert_eq!(
            valid.original_data.get("invalid_case").map(String::as_str),
            Some("anything")
        );
    }

    #[test]
    fn splits_records_by_count_and_names_batches() {
        let records = vec![
            record(2, "m1", "c1"),
            record(3, "m1", "c2"),
            record(4, "m1", "c3"),
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
    fn auto_splits_records_by_byte_size() {
        let records = vec![
            record(2, "merchant_very_long_1", "customer_very_long_1"),
            record(3, "merchant_very_long_2", "customer_very_long_2"),
        ];
        let single_size = records_to_csv_bytes(&records[..1]).unwrap().len();
        let both_size = records_to_csv_bytes(&records).unwrap().len();

        let (batches, invalid) = split_records_into_batches(records, 500, both_size - 1).unwrap();
        assert!(invalid.is_empty());
        assert!(single_size < both_size);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].records.len(), 1);
        assert_eq!(batches[1].records.len(), 1);
    }

    #[test]
    fn status_serializes_as_backend_status_string() {
        let status = CustomerGlobalIdStatus::UpdatedNullId;
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            "\"updated_null_id\""
        );

        let status: CustomerGlobalIdStatus = serde_json::from_str("\"update_failed\"").unwrap();
        assert_eq!(status, CustomerGlobalIdStatus::UpdateFailed);
        assert!(status.is_failed());

        let unknown: CustomerGlobalIdStatus = serde_json::from_str("\"new_status\"").unwrap();
        assert_eq!(unknown.as_str(), "new_status");
        assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"new_status\"");
    }

    #[test]
    fn typed_status_count_map_serializes_as_status_keys() {
        let mut per_status_counts = BTreeMap::new();
        per_status_counts.insert(CustomerGlobalIdStatus::AlreadyGlobalId, 3);
        per_status_counts.insert(CustomerGlobalIdStatus::NotFound, 1);

        let summary = CustomerGlobalIdMigrationSummary {
            total_input_rows: 4,
            valid_rows: 4,
            invalid_input_rows: 0,
            total_batches: 1,
            total_updated_count: 0,
            total_skipped_count: 3,
            total_failed_count: 1,
            per_status_counts,
        };

        let value = serde_json::to_value(summary).unwrap();
        assert_eq!(value["per_status_counts"]["already_global_id"], 3);
        assert_eq!(value["per_status_counts"]["not_found"], 1);
    }
}
