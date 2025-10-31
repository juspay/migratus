use crate::domain::{
    update::{FailedUpdate, SuccessfulUpdate, UpdateBatch, UpdateFailureReason, UpdateMetadata},
};
use crate::error::*;
use reqwest::multipart;
use serde::{Deserialize, Serialize};

pub struct UpdateApiClient {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    merchant_id: String,
    merchant_connector_ids: Option<String>,
}

impl UpdateApiClient {
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

    pub async fn update_batch(&self, batch: &UpdateBatch) -> Result<BatchUpdateResponse> {
        let (response, _headers) = self.update_batch_with_headers(batch).await?;
        Ok(response)
    }

    pub async fn update_batch_with_headers(
        &self,
        batch: &UpdateBatch,
    ) -> Result<(
        BatchUpdateResponse,
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
            let records: Vec<ApiUpdateResponse> = response.json().await?;
            Ok((BatchUpdateResponse::Success(records), headers_map))
        } else {
            let status_code = status.as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error response: {}", e));
            Ok((
                BatchUpdateResponse::Error {
                    status: status_code,
                    message,
                },
                headers_map,
            ))
        }
    }

    fn batch_to_csv(&self, batch: &UpdateBatch) -> Result<Vec<u8>> {
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
        batch: &UpdateBatch,
        response: BatchUpdateResponse,
    ) -> BatchUpdateResult {
        match response {
            BatchUpdateResponse::Success(records) => {
                let mut successful = Vec::new();
                let mut failed = Vec::new();

                for record in records {
                    let line_num = record.line_number.unwrap_or(1) as usize;

                    if record.update_status == "Success" {
                        if let Some(pm_id) = &record.payment_method_id {
                            let original_data = batch
                                .records
                                .get(line_num.saturating_sub(1))
                                .map(|r| r.data.clone())
                                .unwrap_or_default();

                            successful.push(SuccessfulUpdate {
                                line_number: crate::domain::types::LineNumber::new(line_num),
                                batch_number: batch.batch_number,
                                original_data,
                                payment_method_id: crate::domain::types::PaymentMethodId::new(
                                    pm_id.clone(),
                                ),
                                update_status: record.update_status.clone(),
                                metadata: UpdateMetadata {
                                    updated_payment_method_data: record.updated_payment_method_data,
                                    connector_customer_updated: record.connector_customer.is_some(),
                                    connector_mandate_details_updated: record
                                        .connector_mandate_details
                                        .is_some(),
                                },
                            });
                        }
                    } else {
                        let original_data = batch
                            .records
                            .get(line_num.saturating_sub(1))
                            .map(|r| r.data.clone())
                            .unwrap_or_default();

                        failed.push(FailedUpdate {
                            line_number: crate::domain::types::LineNumber::new(line_num),
                            batch_number: batch.batch_number,
                            original_data,
                            failure_reason: UpdateFailureReason::RecordLevelFailure(
                                record.update_error.unwrap_or_default(),
                            ),
                        });
                    }
                }

                BatchUpdateResult { successful, failed }
            }
            BatchUpdateResponse::Error { status, message } => {
                let failed: Vec<FailedUpdate> = batch
                    .records
                    .iter()
                    .enumerate()
                    .map(|(i, record)| FailedUpdate {
                        line_number: crate::domain::types::LineNumber::new(i + 1),
                        batch_number: batch.batch_number,
                        original_data: record.data.clone(),
                        failure_reason: UpdateFailureReason::ApiError {
                            status,
                            message: message.clone(),
                        },
                    })
                    .collect();

                BatchUpdateResult {
                    successful: Vec::new(),
                    failed,
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiUpdateResponse {
    pub payment_method_id: Option<String>,
    pub status: Option<String>,
    pub network_transaction_id: Option<String>,
    pub connector_mandate_details: Option<serde_json::Value>,
    pub update_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_error: Option<String>,
    pub updated_payment_method_data: Option<bool>,
    pub connector_customer: Option<serde_json::Value>,
    pub line_number: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BatchUpdateResponse {
    Success(Vec<ApiUpdateResponse>),
    Error { status: u16, message: String },
}

pub struct BatchUpdateResult {
    pub successful: Vec<SuccessfulUpdate>,
    pub failed: Vec<FailedUpdate>,
}
