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
    pub enrichment: Option<HashMap<String, String>>,
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
            Self::Update => vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataSource {
    Merged { 
        path: PathBuf 
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
    pub merchant_id: String,
    pub merchant_connector_ids: Option<String>,
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

    pub fn merchant_id(&self) -> MerchantId {
        MerchantId::new(self.merchant_id.clone())
    }

    pub fn merchant_connector_ids(&self) -> Option<String> {
        self.merchant_connector_ids.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub resume_from_batch: Option<usize>,
    pub resume_from_state: Option<String>,
}

impl BatchConfig {
    pub fn batch_size(&self) -> Result<BatchSize, String> {
        BatchSize::new(self.batch_size)
    }

    pub fn resume_from_batch(&self) -> Option<BatchNumber> {
        self.resume_from_batch.map(BatchNumber::new)
    }
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
