use crate::domain::config::DataSource;
use crate::error::*;
use crate::machine::builder::*;
use crate::operations::{csv_reader::CsvReader, csv_writer::CsvWriter, merger::Merger};
use crate::states::*;

impl MigrationBuilder<MergeRequired> {
    pub async fn merge(mut self) -> Result<MigrationBuilder<Validated>> {
        let (customer_path, payment_path, merge_on) = match &self.config.data_source {
            DataSource::Separate { customer, payment, merge_on } => (customer, payment, merge_on.clone()),
            _ => {
                return Err(MigrationError::InvalidStateTransition(
                    "MergeRequired state requires separate data sources".to_string(),
                ))
            }
        };

        let mut csv_reader = CsvReader::new();
        let customer_records = csv_reader.read_file(customer_path)?;
        let payment_records = csv_reader.read_file(payment_path)?;

        self.set_original_count(payment_records.len());

        // Validate output_fields against CSV headers
        if let Some(output_fields) = &self.config.output_config.output_fields {
            let csv_headers = csv_reader.get_headers();
            for field in output_fields {
                field
                    .validate_against_csv(&csv_headers)
                    .map_err(MigrationError::ConfigError)?;
            }
        }

        let merger = Merger::new(merge_on);
        let merge_result = merger.merge(customer_records, payment_records);

        self.verify_invariant(merge_result.merged.len(), merge_result.invalid.len())?;

        if !merge_result.invalid.is_empty() {
            // Ensure output directory exists
            std::fs::create_dir_all(&self.config.output_config.output_dir)?;

            let writer = CsvWriter::new();
            let invalid_path = self
                .config
                .output_config
                .output_dir
                .join("invalid_records.csv");
            writer.write_invalid_records(&invalid_path, &merge_result.invalid)?;
        }

        let mut builder = self.transition_to::<Validated>();
        builder.merged_data = Some(merge_result.merged);
        builder.invalid_records = merge_result.invalid;

        Ok(builder)
    }
}

impl MigrationBuilder<MergeSkipped> {
    pub async fn load_merged_data(mut self) -> Result<MigrationBuilder<Validated>> {
        let merged_path = match &self.config.data_source {
            DataSource::Merged { path } => path,
            _ => {
                return Err(MigrationError::InvalidStateTransition(
                    "MergeSkipped state requires merged data source".to_string(),
                ))
            }
        };

        let mut csv_reader = CsvReader::new();
        let records = csv_reader.read_file(merged_path)?;

        self.set_original_count(records.len());

        // Validate output_fields against CSV headers
        if let Some(output_fields) = &self.config.output_config.output_fields {
            let csv_headers = csv_reader.get_headers();
            for field in output_fields {
                field
                    .validate_against_csv(&csv_headers)
                    .map_err(MigrationError::ConfigError)?;
            }
        }

        let merged_records: Vec<_> = records
            .into_iter()
            .map(crate::domain::records::MergedRecord::from_record)
            .collect();

        let mut builder = self.transition_to::<Validated>();
        builder.merged_data = Some(merged_records);

        Ok(builder)
    }
}
