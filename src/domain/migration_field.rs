use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enum representing all possible migration fields (for validation, output, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MigrationField {
    // === API Response Fields (Always Available) ===
    PaymentMethodId,
    MigrationStatus,
    LineNumber,
    BatchNumber,
    PaymentMethod,
    PaymentMethodType,
    CardMigrated,
    NetworkTokenMigrated,
    ConnectorMandateDetailsMigrated,
    NetworkTransactionIdMigrated,
    MigrationError,

    // === Customer Information ===
    CustomerId,
    Name,
    Email,
    Phone,
    PhoneCountryCode,

    // === Merchant Information ===
    MerchantId,
    MerchantConnectorId,
    MerchantConnectorIds,

    // === Payment Method Information ===
    NickName,
    PaymentInstrumentId,
    ConnectorCustomerId,

    // === Card Information ===
    CardNumberMasked,
    CardExpiryMonth,
    CardExpiryYear,
    CardScheme,
    RawCardNumber,

    // === Network Token Information ===
    NetworkTokenNumber,
    NetworkTokenExpiryMonth,
    NetworkTokenExpiryYear,
    NetworkTokenRequestorRefId,

    // === Billing Address ===
    BillingAddressLine1,
    BillingAddressLine2,
    BillingAddressLine3,
    BillingAddressCity,
    BillingAddressState,
    BillingAddressCountry,
    BillingAddressZip,
    BillingAddressFirstName,
    BillingAddressLastName,

    // === Transaction Information ===
    OriginalTransactionId,
    OriginalTransactionAmount,
    OriginalTransactionCurrency,

    // === For custom CSV fields not in enum ===
    #[serde(untagged)]
    Custom(String),
}

impl MigrationField {
    /// Convert field name to snake_case string for CSV header using serde
    pub fn to_header_name(&self) -> String {
        // Use serde to serialize the enum variant name to snake_case
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| {
                // Fallback for Custom variant
                match self {
                    Self::Custom(name) => name.clone(),
                    _ => String::new(),
                }
            })
    }

    /// Validate that Custom fields exist in CSV headers
    pub fn validate_against_csv(&self, csv_headers: &[String]) -> Result<(), String> {
        match self {
            Self::Custom(field_name) => {
                if !csv_headers.contains(field_name) {
                    Err(format!(
                        "Field '{}' not found in CSV headers. Available: {:?}",
                        field_name, csv_headers
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()), // Known fields are always valid
        }
    }

    /// Extract value from original data (CSV) and API response
    pub fn extract_value(
        &self,
        original_data: &HashMap<String, String>,
        api_response: &crate::operations::api_client::ApiMigrationRecord,
        batch_number: usize,
    ) -> String {
        // Special case: batch_number is not in API response
        if matches!(self, Self::BatchNumber) {
            return batch_number.to_string();
        }

        let field_name = self.to_header_name();

        // Convert API response to HashMap using serde
        let api_map: HashMap<String, serde_json::Value> = serde_json::to_value(api_response)
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Try API response first, then CSV
        api_map
            .get(&field_name)
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                serde_json::Value::Null => None,
                _ => Some(v.to_string()),
            })
            .or_else(|| original_data.get(&field_name).cloned())
            .unwrap_or_default()
    }
}
