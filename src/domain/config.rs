use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub flow: MigrationFlow,
    pub data_source: DataSource,
    pub api_config: ApiConfig,
    pub batch_config: BatchConfig,
    pub output_config: OutputConfig,
    #[serde(default)]
    pub enrichment: Option<EnrichmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnrichmentConfig {
    #[serde(flatten)]
    pub values: HashMap<String, serde_json::Value>,
}

impl EnrichmentConfig {
    pub fn string_columns(&self) -> impl Iterator<Item = (&String, &str)> {
        self.values
            .iter()
            .filter(|(key, _)| key.as_str() != "already_migrated")
            .filter_map(|(key, value)| value.as_str().map(|value| (key, value)))
    }

    pub fn already_migrated(&self) -> Option<AlreadyMigratedConfig> {
        self.values
            .get("already_migrated")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlreadyMigratedConfig {
    pub path: PathBuf,
    pub match_fields: Vec<super::migration_field::MigrationField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MigrationFlow {
    RawCard,
    PspToken,
    CustomMigrate {
        required_fields: Vec<super::migration_field::MigrationField>,
        optional_fields: Vec<super::migration_field::MigrationField>,
    },
    CustomerGlobalId,
    #[serde(
        rename = "payment_method_fingerprint",
        alias = "payment_method_fingerprint_id"
    )]
    PaymentMethodFingerprintId,
    Update,
}

impl MigrationFlow {
    pub fn required_fields(&self) -> Vec<super::migration_field::MigrationField> {
        use super::migration_field::MigrationField as MF;

        match self {
            Self::RawCard => vec![
                MF::CustomerId,
                MF::RawCardNumber,
                MF::CardNumberMasked,
                MF::CardExpiryMonth,
                MF::CardExpiryYear,
                MF::PaymentMethod,
            ],
            Self::PspToken => vec![
                MF::CustomerId,
                MF::PaymentInstrumentId,
                MF::CardNumberMasked,
                MF::CardExpiryMonth,
                MF::CardExpiryYear,
            ],
            Self::CustomMigrate {
                required_fields, ..
            } => required_fields.clone(),
            Self::CustomerGlobalId => vec![MF::MerchantId, MF::CustomerId],
            Self::PaymentMethodFingerprintId => vec![MF::MerchantId, MF::PaymentMethodId],
            Self::Update => vec![MF::PaymentMethodId],
        }
    }

    pub fn optional_fields(&self) -> Vec<super::migration_field::MigrationField> {
        use super::migration_field::MigrationField as MF;

        match self {
            Self::RawCard => vec![
                MF::Name,
                MF::Email,
                MF::Phone,
                MF::PhoneCountryCode,
                MF::CardScheme,
                MF::BillingAddressLine1,
                MF::BillingAddressLine2,
                MF::BillingAddressLine3,
                MF::BillingAddressCity,
                MF::BillingAddressState,
                MF::BillingAddressCountry,
                MF::BillingAddressZip,
                MF::BillingAddressFirstName,
                MF::BillingAddressLastName,
            ],
            Self::PspToken => vec![
                MF::Name,
                MF::Email,
                MF::Phone,
                MF::PhoneCountryCode,
                MF::ConnectorCustomerId,
                MF::OriginalTransactionId,
                MF::OriginalTransactionAmount,
                MF::OriginalTransactionCurrency,
                MF::NetworkTokenNumber,
                MF::NetworkTokenExpiryMonth,
                MF::NetworkTokenExpiryYear,
                MF::NetworkTokenRequestorRefId,
            ],
            Self::CustomMigrate {
                optional_fields, ..
            } => optional_fields.clone(),
            Self::CustomerGlobalId => vec![],
            Self::PaymentMethodFingerprintId => vec![],
            Self::Update => vec![],
        }
    }

    pub fn is_customer_global_id(&self) -> bool {
        matches!(self, Self::CustomerGlobalId)
    }

    pub fn is_payment_method_fingerprint_id(&self) -> bool {
        matches!(self, Self::PaymentMethodFingerprintId)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataSource {
    Merged {
        path: PathBuf,
    },
    Separate {
        customer: PathBuf,
        payment: PathBuf,
        #[serde(default = "default_merge_field")]
        merge_on: super::migration_field::MigrationField,
    },
}

fn default_merge_field() -> super::migration_field::MigrationField {
    super::migration_field::MigrationField::CustomerId
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub endpoint: String,
    pub api_key: String,
    #[serde(default)]
    pub merchant_id: Option<String>,
    pub merchant_connector_ids: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

impl ApiConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    pub fn endpoint(&self) -> Result<ApiEndpoint, String> {
        ApiEndpoint::new(self.endpoint.clone())
    }

    pub fn api_key(&self) -> ApiKey {
        ApiKey::new(self.api_key.clone())
    }

    pub fn merchant_id(&self) -> Option<MerchantId> {
        self.merchant_id.clone().map(MerchantId::new)
    }

    pub fn required_merchant_id(&self) -> std::result::Result<String, String> {
        self.merchant_id
            .as_ref()
            .map(|merchant_id| merchant_id.trim())
            .filter(|merchant_id| !merchant_id.is_empty())
            .map(String::from)
            .ok_or_else(|| "api_config.merchant_id is required for this flow".to_string())
    }

    pub fn merchant_connector_ids(&self) -> Option<String> {
        self.merchant_connector_ids.clone()
    }

    pub fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default)]
    pub resume_from_batch: Option<usize>,
    #[serde(default)]
    pub resume_from_state: Option<String>,
    #[serde(default = "default_parallel_uploads")]
    pub parallel_uploads: usize,
    #[serde(default = "default_max_file_size_bytes")]
    pub max_file_size_bytes: usize,
    #[serde(default)]
    pub retry_count: usize,
    #[serde(default = "default_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
}

impl BatchConfig {
    pub fn batch_size(&self) -> Result<BatchSize, String> {
        BatchSize::new(self.batch_size)
    }

    pub fn resume_from_batch(&self) -> Option<BatchNumber> {
        self.resume_from_batch.map(BatchNumber::new)
    }
}

fn default_batch_size() -> usize {
    500
}

fn default_parallel_uploads() -> usize {
    4
}

fn default_max_file_size_bytes() -> usize {
    1_048_576
}

fn default_retry_backoff_ms() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub output_dir: PathBuf,
    pub batch_response_dir: PathBuf,
    #[serde(default)]
    pub output_fields: Option<Vec<super::migration_field::MigrationField>>,
}

impl OutputConfig {
    pub fn output_dir(&self) -> OutputDirectory {
        OutputDirectory::new(self.output_dir.clone())
    }

    pub fn batch_response_dir(&self) -> BatchResponseDirectory {
        BatchResponseDirectory::new(self.batch_response_dir.clone())
    }

    pub fn output_fields(&self) -> Option<&Vec<super::migration_field::MigrationField>> {
        self.output_fields.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentColumns {
    pub columns: HashMap<String, String>,
}

impl EnrichmentColumns {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: String, value: String) {
        self.columns.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.columns.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.columns.iter()
    }
}

impl Default for EnrichmentColumns {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_payment_method_fingerprint_flow_name_and_alias() {
        let flow: MigrationFlow =
            serde_json::from_str(r#"{"type":"payment_method_fingerprint"}"#).unwrap();
        assert!(flow.is_payment_method_fingerprint_id());

        let alias: MigrationFlow =
            serde_json::from_str(r#"{"type":"payment_method_fingerprint_id"}"#).unwrap();
        assert!(alias.is_payment_method_fingerprint_id());
    }

    #[test]
    fn api_merchant_id_is_optional_but_validated_when_required() {
        let config: ApiConfig = serde_json::from_str(
            r#"{
                "endpoint": "https://example.com",
                "api_key": "secret",
                "merchant_connector_ids": null
            }"#,
        )
        .unwrap();

        assert!(config.merchant_id().is_none());
        assert!(config.required_merchant_id().is_err());

        let config: ApiConfig = serde_json::from_str(
            r#"{
                "endpoint": "https://example.com",
                "api_key": "secret",
                "merchant_id": "merchant_123",
                "merchant_connector_ids": null
            }"#,
        )
        .unwrap();

        assert_eq!(config.required_merchant_id().unwrap(), "merchant_123");
    }
}
