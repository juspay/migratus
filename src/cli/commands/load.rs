use crate::domain::config::{DataSource, MigrationConfig};
use crate::domain::records::MergedRecord;
use crate::operations::csv_reader::CsvReader;
use crate::operations::csv_writer::CsvWriter;
use crate::operations::merger::Merger;
use crate::utils::hash::calculate_config_hash;
use crate::utils::intermediate::IntermediateOutput;
use std::fs;
use std::path::Path;

pub async fn handle_load(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("📥 LOAD Stage");
    println!("=============\n");

    // Calculate config hash
    let config_hash = calculate_config_hash(config_path)?;

    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;

    if crate::cli::commands::customer_global_id::is_customer_global_id_config(&config) {
        return crate::cli::commands::customer_global_id::handle_load(config_path).await;
    }

    // Ensure output directory exists
    fs::create_dir_all(&config.output_config.output_dir)?;

    let merged_records = match &config.data_source {
        DataSource::Separate {
            customer,
            payment,
            merge_on,
        } => {
            println!("📄 Loading separate files:");
            println!("  - Customer: {}", customer.display());
            println!("  - Payment: {}", payment.display());

            // Read files
            let mut csv_reader = CsvReader::new();
            let customer_records = csv_reader.read_file(customer)?;
            let payment_records = csv_reader.read_file(payment)?;

            println!("  ✓ Customer records: {}", customer_records.len());
            println!("  ✓ Payment records: {}", payment_records.len());
            println!();

            // Merge
            println!("🔀 Merging records on {}...", merge_on.to_header_name());
            let merger = Merger::new(merge_on.clone());
            let merge_result = merger.merge(customer_records, payment_records);

            println!("  ✓ Merged: {}", merge_result.merged.len());
            println!("  ✗ Invalid: {}", merge_result.invalid.len());

            // Save invalid records if any
            if !merge_result.invalid.is_empty() {
                let invalid_path = config.output_config.output_dir.join("invalid_records.csv");
                let writer = CsvWriter::new();
                writer.write_invalid_records(&invalid_path, &merge_result.invalid)?;
                println!("  → Invalid records saved to: {}", invalid_path.display());
            }
            println!();

            merge_result.merged
        }
        DataSource::Merged { path } => {
            println!("📄 Loading merged file: {}", path.display());

            let mut csv_reader = CsvReader::new();
            let records = csv_reader.read_file(path)?;

            println!("  ✓ Records loaded: {}", records.len());
            println!();

            // Convert to MergedRecord
            records.into_iter().map(MergedRecord::from_record).collect()
        }
    };

    // Wrap in IntermediateOutput
    let output = IntermediateOutput::new(config_hash, merged_records);

    // Save to JSON
    let output_path = config.output_config.output_dir.join("merged_records.json");
    let json = serde_json::to_string_pretty(&output)?;
    fs::write(&output_path, json)?;

    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!("  → {} records", output.record_count);
    println!("  → Config hash: {}...", &output.config_hash[..8]);
    println!();

    println!("✅ LOAD stage complete!");
    println!();
    println!("Next step:");
    println!("  migratus validate {}", config_path.display());

    Ok(())
}
