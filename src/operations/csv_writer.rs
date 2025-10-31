use crate::domain::records::{FailedMigration, InvalidRecord, SuccessfulMigration};
use crate::domain::update::{FailedUpdate, SuccessfulUpdate};
use crate::error::*;
use std::path::Path;

pub struct CsvWriter;

impl CsvWriter {
    pub fn new() -> Self {
        Self
    }

    pub fn write_invalid_records(&self, path: &Path, records: &[InvalidRecord]) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        writer.write_record([
            "line_number",
            "invalid_reason",
            "failed_at_state",
            "data",
        ])?;

        for record in records {
            let data_json = serde_json::to_string(&record.original_data)?;
            writer.write_record(&[
                record.line_number.value().to_string(),
                record.invalid_reason.to_string(),
                record.failed_at_state.as_str().to_string(),
                data_json,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn write_successful_migrations(
        &self,
        path: &Path,
        migrations: &[SuccessfulMigration],
    ) -> Result<()> {
        self.write_successful_migrations_default(path, migrations)
    }

    fn write_successful_migrations_default(
        &self,
        path: &Path,
        migrations: &[SuccessfulMigration],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        writer.write_record([
            "line_number",
            "batch_number",
            "payment_method_id",
            "customer_id",
            "migration_status",
            "original_data",
        ])?;

        for migration in migrations {
            let data_json = serde_json::to_string(&migration.original_data)?;
            writer.write_record(&[
                migration.line_number.value().to_string(),
                migration.batch_number.value().to_string(),
                migration.payment_method_id.inner().to_string(),
                migration.customer_id.to_string(),
                migration.migration_status.clone(),
                data_json,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn write_successful_migrations_custom(
        &self,
        path: &Path,
        migrations: &[SuccessfulMigration],
        output_fields: &[crate::domain::migration_field::MigrationField],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        // Write headers
        let headers: Vec<String> = output_fields.iter().map(|f| f.to_header_name()).collect();
        writer.write_record(&headers)?;

        // Write data rows
        for migration in migrations {
            // Create a mock API record for field extraction
            let api_record = crate::operations::api_client::ApiMigrationRecord {
                line_number: Some(migration.line_number.value() as i64),
                payment_method_id: Some(migration.payment_method_id.inner().to_string()),
                payment_method: migration.payment_method.clone(),
                payment_method_type: migration.payment_method_type.clone(),
                customer_id: Some(migration.customer_id.to_string()),
                migration_status: migration.migration_status.clone(),
                migration_error: None,
                card_number_masked: migration.card_number_masked.clone(),
                card_migrated: migration.metadata.card_migrated,
                network_token_migrated: Some(migration.metadata.network_token_migrated),
                connector_mandate_details_migrated: Some(
                    migration.metadata.connector_mandate_details_migrated,
                ),
                network_transaction_id_migrated: Some(
                    migration.metadata.network_transaction_id_migrated,
                ),
            };

            let values: Vec<String> = output_fields
                .iter()
                .map(|field| {
                    field.extract_value(
                        &migration.original_data,
                        &api_record,
                        migration.batch_number.value(),
                    )
                })
                .collect();

            writer.write_record(&values)?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn write_failed_migrations(
        &self,
        path: &Path,
        migrations: &[FailedMigration],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        writer.write_record([
            "line_number",
            "batch_number",
            "failure_reason",
            "original_data",
        ])?;

        for migration in migrations {
            let data_json = serde_json::to_string(&migration.original_data)?;
            writer.write_record(&[
                migration.line_number.value().to_string(),
                migration.batch_number.value().to_string(),
                migration.failure_reason.to_string(),
                data_json,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    // Update flow methods
    pub fn write_successful_updates(
        &self,
        path: &Path,
        updates: &[SuccessfulUpdate],
    ) -> Result<()> {
        self.write_successful_updates_default(path, updates)
    }

    fn write_successful_updates_default(
        &self,
        path: &Path,
        updates: &[SuccessfulUpdate],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        writer.write_record([
            "line_number",
            "batch_number",
            "payment_method_id",
            "update_status",
            "original_data",
        ])?;

        for update in updates {
            let data_json = serde_json::to_string(&update.original_data)?;
            writer.write_record(&[
                update.line_number.value().to_string(),
                update.batch_number.value().to_string(),
                update.payment_method_id.inner().to_string(),
                update.update_status.clone(),
                data_json,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn write_successful_updates_custom(
        &self,
        path: &Path,
        updates: &[SuccessfulUpdate],
        output_fields: &[crate::domain::update::UpdateField],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        // Write headers
        let headers: Vec<String> = output_fields.iter().map(|f| f.to_header_name()).collect();
        writer.write_record(&headers)?;

        // Write data rows
        for update in updates {
            // Create a mock API record for field extraction
            let api_record = crate::operations::api::ApiUpdateResponse {
                line_number: Some(update.line_number.value() as i64),
                payment_method_id: Some(update.payment_method_id.inner().to_string()),
                status: None,
                network_transaction_id: None,
                connector_mandate_details: None,
                update_status: update.update_status.clone(),
                update_error: None,
                updated_payment_method_data: update.metadata.updated_payment_method_data,
                connector_customer: None,
            };

            let values: Vec<String> = output_fields
                .iter()
                .map(|field| {
                    field.extract_value(
                        &update.original_data,
                        &api_record,
                        update.batch_number.value(),
                    )
                })
                .collect();

            writer.write_record(&values)?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn write_failed_updates(
        &self,
        path: &Path,
        updates: &[FailedUpdate],
    ) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)?;

        writer.write_record([
            "line_number",
            "batch_number",
            "failure_reason",
            "original_data",
        ])?;

        for update in updates {
            let data_json = serde_json::to_string(&update.original_data)?;
            writer.write_record(&[
                update.line_number.value().to_string(),
                update.batch_number.value().to_string(),
                update.failure_reason.to_string(),
                data_json,
            ])?;
        }

        writer.flush()?;
        Ok(())
    }
}

impl Default for CsvWriter {
    fn default() -> Self {
        Self::new()
    }
}
