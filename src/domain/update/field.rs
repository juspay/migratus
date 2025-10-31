use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enum representing all possible update fields (for validation, output, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum UpdateField {
    // === Required Field ===
    PaymentMethodId,

    // === Optional Update Fields ===
    Status,
    NetworkTransactionId,
    PaymentInstrumentId,
    ConnectorCustomerId,
    MerchantConnectorIds,
    CardExpiryMonth,
    CardExpiryYear,

    // === API Response Fields ===
    UpdateStatus,
    UpdateError,
    UpdatedPaymentMethodData,
    ConnectorCustomer,
    ConnectorMandateDetails,
    LineNumber,
    BatchNumber,

    // === For custom CSV fields not in enum ===
    #[serde(untagged)]
    Custom(String),
}

impl UpdateField {
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
        api_response: &crate::operations::api::update::ApiUpdateResponse,
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

    /// Get list of required fields for update flow
    pub fn required_fields() -> Vec<Self> {
        vec![Self::PaymentMethodId]
    }

    /// Get list of optional update fields
    pub fn optional_update_fields() -> Vec<Self> {
        vec![
            Self::Status,
            Self::NetworkTransactionId,
            Self::PaymentInstrumentId,
            Self::ConnectorCustomerId,
            Self::MerchantConnectorIds,
            Self::CardExpiryMonth,
            Self::CardExpiryYear,
        ]
    }

    /// Check if this is an optional update field
    pub fn is_optional_update_field(&self) -> bool {
        matches!(
            self,
            Self::Status
                | Self::NetworkTransactionId
                | Self::PaymentInstrumentId
                | Self::ConnectorCustomerId
                | Self::MerchantConnectorIds
                | Self::CardExpiryMonth
                | Self::CardExpiryYear
        )
    }
}
