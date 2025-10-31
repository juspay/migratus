use crate::domain::records::MigrationResults;
use crate::error::*;
use crate::machine::builder::*;
use crate::operations::api_client::ApiClient;
use crate::states::{Completed, Migrated};
use std::fs;

impl MigrationBuilder<Migrated> {
    pub async fn migrate(mut self) -> Result<MigrationBuilder<Completed>> {
        let batches = self
            .batches
            .take()
            .ok_or_else(|| MigrationError::InternalError("No batches".to_string()))?;

        let api_client = ApiClient::new(
            self.config.api_config.endpoint.clone(),
            self.config.api_config.api_key.clone(),
            self.config.api_config.merchant_id.clone(),
            self.config.api_config.merchant_connector_ids.clone(),
            self.config.api_config.timeout(),
        )?;

        let batch_response_dir = self.config.output_config.batch_response_dir();
        fs::create_dir_all(batch_response_dir.path())?;

        let mut migration_results = MigrationResults::new();

        for batch in batches {
            let (response, headers) = api_client.migrate_batch_with_headers(&batch).await?;

            // Save response with headers
            let response_path = batch_response_dir
                .path()
                .join(format!("batch_{:04}.json", batch.batch_number.value()));

            let response_data = serde_json::json!({
                "batch_number": batch.batch_number.value(),
                "headers": headers,
                "body": response
            });

            let response_json = serde_json::to_string_pretty(&response_data)?;
            fs::write(&response_path, response_json)?;

            let batch_result = api_client.parse_response(&batch, response);
            migration_results.successful.extend(batch_result.successful);
            migration_results.failed.extend(batch_result.failed);
        }

        let mut builder = self.transition_to::<Completed>();
        builder.migration_results = Some(migration_results);

        Ok(builder)
    }
}
