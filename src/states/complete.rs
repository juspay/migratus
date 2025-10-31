use crate::domain::records::MigrationSummary;
use crate::error::*;
use crate::machine::builder::*;
use crate::operations::csv_writer::CsvWriter;
use crate::states::Completed;

impl MigrationBuilder<Completed> {
    pub async fn complete(self) -> Result<FinalOutput> {
        let migration_results = self
            .migration_results
            .as_ref()
            .ok_or_else(|| MigrationError::InternalError("No migration results".to_string()))?;

        let output_dir = self.config.output_config.output_dir();
        std::fs::create_dir_all(output_dir.path())?;

        let writer = CsvWriter::new();

        if !migration_results.successful.is_empty() {
            let success_path = output_dir.path().join("successful_migrations.csv");

            // Use custom output fields if specified, otherwise use default
            if let Some(output_fields) = self.config.output_config.output_fields() {
                writer.write_successful_migrations_custom(
                    &success_path,
                    &migration_results.successful,
                    output_fields,
                )?;
            } else {
                writer.write_successful_migrations(&success_path, &migration_results.successful)?;
            }
        }

        if !migration_results.failed.is_empty() {
            let failed_path = output_dir.path().join("failed_migrations.csv");
            writer.write_failed_migrations(&failed_path, &migration_results.failed)?;
        }

        if !self.invalid_records.is_empty() {
            let invalid_path = output_dir.path().join("invalid_records.csv");
            writer.write_invalid_records(&invalid_path, &self.invalid_records)?;
        }

        let summary = MigrationSummary {
            total_input_records: self.original_input_count,
            valid_for_migration: migration_results.total(),
            invalid_pre_migration: self.invalid_records.len(),
            successful_migrations: migration_results.successful.len(),
            failed_migrations: migration_results.failed.len(),
            total_output_records: migration_results.total() + self.invalid_records.len(),
            invalid_at_merge: self
                .invalid_records
                .iter()
                .filter(|r| matches!(r.failed_at_state, crate::domain::records::StateName::Merge))
                .count(),
            invalid_at_validation: self
                .invalid_records
                .iter()
                .filter(|r| {
                    matches!(
                        r.failed_at_state,
                        crate::domain::records::StateName::Validation
                    )
                })
                .count(),
        };

        summary
            .verify_invariant()
            .map_err(MigrationError::InternalError)?;

        Ok(FinalOutput {
            successful_migrations: migration_results.successful.clone(),
            failed_migrations: migration_results.failed.clone(),
            invalid_records: self.invalid_records.clone(),
            summary,
        })
    }
}
