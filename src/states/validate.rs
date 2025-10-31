use crate::error::*;
use crate::machine::builder::*;
use crate::operations::{csv_writer::CsvWriter, validator::Validator};
use crate::states::*;

impl MigrationBuilder<Validated> {
    pub async fn validate(mut self) -> Result<MigrationBuilder<Enriched>> {
        let merged_data = self
            .merged_data
            .take()
            .ok_or_else(|| MigrationError::InternalError("No merged data".to_string()))?;

        let validator = Validator::from_flow(&self.config.flow);

        // First, filter out duplicates
        let (non_duplicate_records, duplicate_invalids) = validator.filter_duplicates(merged_data);
        self.invalid_records.extend(duplicate_invalids);

        // Then validate the remaining records
        let validation_result = validator.validate(non_duplicate_records);
        self.invalid_records.extend(validation_result.invalid);

        self.verify_invariant(validation_result.valid.len(), self.invalid_records.len())?;

        if !self.invalid_records.is_empty() {
            // Ensure output directory exists
            std::fs::create_dir_all(&self.config.output_config.output_dir)?;

            let writer = CsvWriter::new();
            let invalid_path = self
                .config
                .output_config
                .output_dir
                .join("invalid_records.csv");
            writer.write_invalid_records(&invalid_path, &self.invalid_records)?;
        }

        let mut builder = self.transition_to::<Enriched>();
        builder.enriched_data = Some(validation_result.valid);

        Ok(builder)
    }
}
