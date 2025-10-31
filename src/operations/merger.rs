use crate::domain::{
    migration_field::MigrationField,
    records::{InvalidReason, InvalidRecord, MergedRecord, Record, StateName},
};
use std::collections::{HashMap, HashSet};

pub struct Merger {
    customer_id_field: MigrationField,
}

impl Merger {
    pub fn new(customer_id_field: MigrationField) -> Self {
        Self { customer_id_field }
    }

    pub fn merge(
        &self,
        customer_records: Vec<Record>,
        payment_records: Vec<Record>,
    ) -> MergeResult {
        let mut merged = Vec::new();
        let mut invalid = Vec::new();

        let mut unmatched_customers = Vec::new();
        let mut unmatched_payments = Vec::new();

        let min_len = customer_records.len().min(payment_records.len());

        for i in 0..min_len {
            let customer = &customer_records[i];
            let payment = &payment_records[i];

            let customer_id = customer.get(&self.customer_id_field);
            let payment_id = payment.get(&self.customer_id_field);

            if customer_id == payment_id && customer_id.is_some() {
                let merged_data = self.merge_data(&customer.data, &payment.data);
                merged.push(MergedRecord::new(payment.line_number, merged_data));
            } else {
                unmatched_customers.push(customer.clone());
                unmatched_payments.push(payment.clone());
            }
        }

        if customer_records.len() > min_len {
            unmatched_customers.extend(customer_records[min_len..].iter().cloned());
        }
        if payment_records.len() > min_len {
            unmatched_payments.extend(payment_records[min_len..].iter().cloned());
        }

        // For ID-based merge, use ALL customers (not just unmatched) to support one-to-many
        let (id_merged, _remaining_customers, remaining_payments) =
            self.id_based_merge(customer_records, unmatched_payments);
        merged.extend(id_merged);

        for payment in remaining_payments {
            invalid.push(InvalidRecord {
                line_number: payment.line_number,
                original_data: payment.data,
                invalid_reason: InvalidReason::MergeFailure(
                    "No matching customer record".to_string(),
                ),
                failed_at_state: StateName::Merge,
            });
        }

        MergeResult { merged, invalid }
    }

    fn id_based_merge(
        &self,
        customers: Vec<Record>,
        payments: Vec<Record>,
    ) -> (Vec<MergedRecord>, Vec<Record>, Vec<Record>) {
        let mut customer_map: HashMap<String, Record> = HashMap::new();
        for customer in customers {
            if let Some(id) = customer.get(&self.customer_id_field) {
                customer_map.insert(id.clone(), customer);
            }
        }

        let mut merged = Vec::new();
        let mut unmatched_payments = Vec::new();

        for payment in payments {
            if let Some(id) = payment.get(&self.customer_id_field) {
                if let Some(customer) = customer_map.get(id) {
                    let merged_data = self.merge_data(&customer.data, &payment.data);
                    merged.push(MergedRecord::new(payment.line_number, merged_data));
                } else {
                    unmatched_payments.push(payment);
                }
            } else {
                unmatched_payments.push(payment);
            }
        }

        let remaining_customers: Vec<Record> = customer_map.into_values().collect();

        (merged, remaining_customers, unmatched_payments)
    }

    fn merge_data(
        &self,
        customer_data: &HashMap<String, String>,
        payment_data: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut merged = customer_data.clone();
        for (key, value) in payment_data {
            merged.insert(key.clone(), value.clone());
        }
        merged
    }

    pub fn detect_duplicates(&self, records: &[Record]) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut duplicates = HashSet::new();

        for record in records {
            if let Some(id) = record.get(&self.customer_id_field) {
                if !seen.insert(id.clone()) {
                    duplicates.insert(id.clone());
                }
            }
        }

        duplicates
    }
}

pub struct MergeResult {
    pub merged: Vec<MergedRecord>,
    pub invalid: Vec<InvalidRecord>,
}

impl Default for Merger {
    fn default() -> Self {
        Self::new(MigrationField::CustomerId)
    }
}
