use crate::domain::{
    records::{Batch, FailedMigration, MigrationFailureReason, SuccessfulMigration},
    types::{CustomerId, PaymentMethodId},
};
use crate::error::*;
use reqwest::multipart;
use serde::{Deserialize, Serialize};

pub struct ApiClient {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    merchant_id: String,
    merchant_connector_ids: Option<String>,
}

impl ApiClient {
    pub fn new(
        endpoint: String,
        api_key: String,
        merchant_id: String,
        merchant_connector_ids: Option<String>,
        timeout: std::time::Duration,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()?;

        Ok(Self {
            client,
            endpoint,
            api_key,
            merchant_id,
            merchant_connector_ids,
        })
    }

    pub async fn migrate_batch(&self, batch: &Batch) -> Result<BatchMigrationResponse> {
        let (response, _headers) = self.migrate_batch_with_headers(batch).await?;
        Ok(response)
    }

    pub async fn migrate_batch_with_headers(
        &self,
        batch: &Batch,
    ) -> Result<(
        BatchMigrationResponse,
        std::collections::HashMap<String, String>,
    )> {
        let csv_data = self.batch_to_csv(batch)?;

        let mut form = multipart::Form::new()
            .text("merchant_id", self.merchant_id.clone())
            .part(
                "file",
                multipart::Part::bytes(csv_data)
                    .file_name(format!("batch_{}.csv", batch.batch_number.value()))
                    .mime_str("text/csv")?,
            );

        if let Some(mca_ids) = &self.merchant_connector_ids {
            form = form.text("merchant_connector_ids", mca_ids.clone());
        }

        let response = self
            .client
            .post(&self.endpoint)
            .header("api-key", &self.api_key)
            .multipart(form)
            .send()
            .await?;

        // Capture headers
        let mut headers_map = std::collections::HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers_map.insert(key.to_string(), value_str.to_string());
            }
        }

        let status = response.status();

        if status.is_success() {
            let records: Vec<ApiMigrationRecord> = response.json().await?;
            Ok((BatchMigrationResponse::Success(records), headers_map))
        } else {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            Ok((
                BatchMigrationResponse::Error {
                    status: status_code,
                    message,
                },
                headers_map,
            ))
        }
    }

    fn batch_to_csv(&self, batch: &Batch) -> Result<Vec<u8>> {
        let mut wtr = csv::Writer::from_writer(vec![]);

        if let Some(first_record) = batch.records.first() {
            let headers: Vec<String> = first_record.data.keys().cloned().collect();
            wtr.write_record(&headers)?;

            for record in &batch.records {
                let values: Vec<String> = headers
                    .iter()
                    .map(|h| record.data.get(h).cloned().unwrap_or_default())
                    .collect();
                wtr.write_record(&values)?;
            }
        }

        wtr.into_inner()
            .map_err(|e| MigrationError::InternalError(format!("CSV writer error: {}", e)))
    }

    pub fn parse_response(
        &self,
        batch: &Batch,
        response: BatchMigrationResponse,
    ) -> BatchMigrationResult {
        match response {
            BatchMigrationResponse::Success(records) => {
                let mut successful = Vec::new();
                let mut failed = Vec::new();

                for record in records {
                    let line_num = record.line_number.unwrap_or(1) as usize;

                    if record.migration_status == "Success" {
                        if let (Some(pm_id), Some(cust_id)) =
                            (&record.payment_method_id, &record.customer_id)
                        {
                            let original_data = batch
                                .records
                                .get(line_num.saturating_sub(1))
                                .map(|r| r.data.clone())
                                .unwrap_or_default();

                            successful.push(SuccessfulMigration {
                                line_number: crate::domain::types::LineNumber::new(line_num),
                                batch_number: batch.batch_number,
                                original_data,
                                payment_method_id: PaymentMethodId::new(pm_id.clone()),
                                customer_id: CustomerId::new(cust_id.clone()),
                                migration_status: record.migration_status.clone(),
                                metadata: crate::domain::records::MigrationMetadata {
                                    card_migrated: record.card_migrated,
                                    network_token_migrated: record
                                        .network_token_migrated
                                        .unwrap_or(false),
                                    connector_mandate_details_migrated: record
                                        .connector_mandate_details_migrated
                                        .unwrap_or(false),
                                    network_transaction_id_migrated: record
                                        .network_transaction_id_migrated
                                        .unwrap_or(false),
                                },
                                payment_method: record.payment_method.clone(),
                                payment_method_type: record.payment_method_type.clone(),
                                card_number_masked: record.card_number_masked.clone(),
                            });
                        }
                    } else {
                        let original_data = batch
                            .records
                            .get(line_num.saturating_sub(1))
                            .map(|r| r.data.clone())
                            .unwrap_or_default();

                        failed.push(FailedMigration {
                            line_number: crate::domain::types::LineNumber::new(line_num),
                            batch_number: batch.batch_number,
                            original_data,
                            failure_reason: MigrationFailureReason::RecordLevelFailure(
                                record.migration_error.unwrap_or_default(),
                            ),
                        });
                    }
                }

                BatchMigrationResult { successful, failed }
            }
            BatchMigrationResponse::Error { status, message } => {
                let failed: Vec<FailedMigration> = batch
                    .records
                    .iter()
                    .enumerate()
                    .map(|(i, record)| FailedMigration {
                        line_number: crate::domain::types::LineNumber::new(i + 1),
                        batch_number: batch.batch_number,
                        original_data: record.data.clone(),
                        failure_reason: MigrationFailureReason::ApiError {
                            status,
                            message: message.clone(),
                        },
                    })
                    .collect();

                BatchMigrationResult {
                    successful: Vec::new(),
                    failed,
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiMigrationRecord {
    pub line_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_type: Option<String>,
    pub customer_id: Option<String>,
    pub migration_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_error: Option<String>,
    pub card_number_masked: Option<String>,
    pub card_migrated: Option<bool>,
    pub network_token_migrated: Option<bool>,
    pub connector_mandate_details_migrated: Option<bool>,
    pub network_transaction_id_migrated: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchMigrationResponse {
    Success(Vec<ApiMigrationRecord>),
    Error { status: u16, message: String },
}

pub struct BatchMigrationResult {
    pub successful: Vec<SuccessfulMigration>,
    pub failed: Vec<FailedMigration>,
}
