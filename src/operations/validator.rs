use crate::domain::{
    config::MigrationFlow,
    migration_field::MigrationField as MF,
    records::{EnrichedRecord, InvalidReason, InvalidRecord, MergedRecord, StateName},
};
use std::collections::HashSet;

pub struct Validator {
    required_fields: Vec<MF>,
}

impl Validator {
    pub fn from_flow(flow: &MigrationFlow) -> Self {
        Self {
            required_fields: flow.required_fields(),
        }
    }

    pub fn validate(&self, records: Vec<MergedRecord>) -> ValidationResult {
        let mut valid = Vec::new();
        let mut invalid = Vec::new();

        for record in records {
            match self.validate_record(&record) {
                Ok(_) => {
                    valid.push(EnrichedRecord::from_merged(record));
                }
                Err(reason) => {
                    invalid.push(InvalidRecord {
                        line_number: record.line_number,
                        original_data: record.data,
                        invalid_reason: reason,
                        failed_at_state: StateName::Validation,
                    });
                }
            }
        }

        ValidationResult { valid, invalid }
    }

    fn validate_record(&self, record: &MergedRecord) -> Result<(), InvalidReason> {
        // Validate required fields
        for field in &self.required_fields {
            let field_name = field.to_header_name();
            if let Some(value) = record.data.get(&field_name) {
                if value.trim().is_empty() {
                    return Err(InvalidReason::MissingRequiredField(field_name));
                }
            } else {
                return Err(InvalidReason::MissingRequiredField(field_name));
            }
        }

        // For Update flow: ensure at least one optional update field is present
        if self.required_fields.len() == 1 && self.required_fields[0] == MF::PaymentMethodId {
            let optional_update_fields = [
                // Card fields
                MF::CardExpiryMonth,
                MF::CardExpiryYear,
                // Connector fields
                MF::PaymentInstrumentId,
                MF::ConnectorCustomerId,
            ];

            let has_update_field = optional_update_fields.iter().any(|field| {
                record
                    .data
                    .get(&field.to_header_name())
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            });

            if !has_update_field {
                return Err(InvalidReason::MissingRequiredField(
                    "At least one update field (e.g., card_expiry_month, payment_instrument_id, connector_customer_id)".to_string()
                ));
            }
        }

        if let Some(email) = record.get_field(&MF::Email) {
            if !email.is_empty() && !email.contains('@') {
                return Err(InvalidReason::InvalidFormat {
                    field: MF::Email.to_header_name(),
                    message: "Invalid email format".to_string(),
                });
            }
        }

        if let Some(month) = record.get_field(&MF::CardExpiryMonth) {
            if !month.is_empty() {
                if let Ok(m) = month.parse::<u8>() {
                    if !(1..=12).contains(&m) {
                        return Err(InvalidReason::InvalidFormat {
                            field: MF::CardExpiryMonth.to_header_name(),
                            message: "Month must be between 1 and 12".to_string(),
                        });
                    }
                } else {
                    return Err(InvalidReason::InvalidFormat {
                        field: MF::CardExpiryMonth.to_header_name(),
                        message: "Invalid month format".to_string(),
                    });
                }
            }
        }

        if let Some(year) = record.get_field(&MF::CardExpiryYear) {
            if !year.is_empty() && year.len() != 4 {
                return Err(InvalidReason::InvalidFormat {
                    field: MF::CardExpiryYear.to_header_name(),
                    message: "Year must be 4 digits".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Build all possible duplicate detection keys for a record
    fn build_duplicate_keys(record: &MergedRecord) -> Vec<String> {
        let mut keys = Vec::new();

        let customer_id = match record.get_field(&MF::CustomerId) {
            Some(id) if !id.is_empty() => id,
            _ => return keys, // No customer_id, can't build keys
        };

        let expiry_month = record.get_field(&MF::CardExpiryMonth);
        let expiry_year = record.get_field(&MF::CardExpiryYear);

        // Key 1: Masked card + expiry (if available)
        if let Some(masked) = record.get_field(&MF::CardNumberMasked) {
            if !masked.is_empty() {
                if let (Some(month), Some(year)) = (expiry_month, expiry_year) {
                    if !month.is_empty() && !year.is_empty() {
                        keys.push(format!(
                            "masked|{}|{}|{}|{}",
                            customer_id, masked, month, year
                        ));
                    }
                }
            }
        }

        // Key 2: Raw card + masked + expiry (if available)
        if let Some(raw) = record.get_field(&MF::RawCardNumber) {
            if !raw.is_empty() {
                // For RawCard flow, card_number_masked is required, so we can safely get it
                if let Some(masked) = record.get_field(&MF::CardNumberMasked) {
                    if !masked.is_empty() {
                        if let (Some(month), Some(year)) = (expiry_month, expiry_year) {
                            if !month.is_empty() && !year.is_empty() {
                                keys.push(format!(
                                    "raw|{}|{}|{}|{}|{}",
                                    customer_id, raw, masked, month, year
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Key 3: Payment instrument ID (if available)
        if let Some(instrument_id) = record.get_field(&MF::PaymentInstrumentId) {
            if !instrument_id.is_empty() {
                keys.push(format!("instrument|{}|{}", customer_id, instrument_id));
            }
        }

        keys
    }

    pub fn detect_duplicates(&self, records: &[MergedRecord]) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut duplicates = HashSet::new();

        for record in records {
            let keys = Self::build_duplicate_keys(record);

            for key in keys {
                if !seen.insert(key.clone()) {
                    duplicates.insert(key);
                }
            }
        }

        duplicates
    }

    pub fn filter_duplicates(
        &self,
        records: Vec<MergedRecord>,
    ) -> (Vec<MergedRecord>, Vec<InvalidRecord>) {
        let duplicate_keys = self.detect_duplicates(&records);
        let mut valid = Vec::new();
        let mut invalid = Vec::new();
        let mut seen_first_occurrence = std::collections::HashSet::new();

        for record in records {
            let record_keys = Self::build_duplicate_keys(&record);

            // Check if ANY of this record's keys is a duplicate
            let matching_duplicate_key =
                record_keys.iter().find(|key| duplicate_keys.contains(*key));

            if let Some(dup_key) = matching_duplicate_key {
                // Check if this is the first occurrence of this duplicate
                if !seen_first_occurrence.contains(dup_key) {
                    // First occurrence - keep it as valid
                    seen_first_occurrence.insert(dup_key.clone());
                    valid.push(record);
                } else {
                    // Subsequent occurrence - mark as invalid
                    let customer_id = record
                        .get_field(&MF::CustomerId)
                        .map(|s| s.as_str())
                        .unwrap_or("unknown");

                    // Build descriptive error message based on which key matched
                    let error_msg = if dup_key.starts_with("masked|") {
                        let card = record.get_field(&MF::CardNumberMasked).unwrap();
                        let month = record.get_field(&MF::CardExpiryMonth).unwrap();
                        let year = record.get_field(&MF::CardExpiryYear).unwrap();
                        format!(
                            "Duplicate payment record (customer: {}, card: {}, expiry: {}/{})",
                            customer_id, card, month, year
                        )
                    } else if dup_key.starts_with("raw|") {
                        let month = record.get_field(&MF::CardExpiryMonth).unwrap();
                        let year = record.get_field(&MF::CardExpiryYear).unwrap();
                        format!(
                            "Duplicate payment record (customer: {}, raw card with expiry: {}/{})",
                            customer_id, month, year
                        )
                    } else if dup_key.starts_with("instrument|") {
                        let instrument = record.get_field(&MF::PaymentInstrumentId).unwrap();
                        format!(
                            "Duplicate payment record (customer: {}, instrument: {})",
                            customer_id, instrument
                        )
                    } else {
                        format!("Duplicate payment record (customer: {})", customer_id)
                    };

                    invalid.push(InvalidRecord {
                        line_number: record.line_number,
                        original_data: record.data,
                        invalid_reason: InvalidReason::DuplicateRecord(error_msg),
                        failed_at_state: StateName::Validation,
                    });
                }
            } else {
                // Not a duplicate - keep for other validation
                valid.push(record);
            }
        }

        (valid, invalid)
    }
}

pub struct ValidationResult {
    pub valid: Vec<EnrichedRecord>,
    pub invalid: Vec<InvalidRecord>,
}
