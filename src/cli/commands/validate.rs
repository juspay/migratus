use crate::domain::config::MigrationConfig;
use crate::domain::records::MergedRecord;
use crate::operations::validator::Validator;
use crate::operations::csv_writer::CsvWriter;
use crate::utils::hash::{calculate_config_hash, verify_config_hash};
use crate::utils::intermediate::IntermediateOutput;
use std::fs;
use std::path::Path;

pub async fn handle_validate(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ VALIDATE Stage");
    println!("=================\n");
    
    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;
    
    // Read merged records
    let merged_path = config.output_config.output_dir.join("merged_records.json");
    if !merged_path.exists() {
        return Err(format!(
            "Merged records not found. Run 'migratus load {}' first",
            config_path.display()
        )
        .into());
    }
    
    let merged_json = fs::read_to_string(&merged_path)?;
    
    // Verify config hash
    if !force && !verify_config_hash(&merged_json, config_path)? {
        return Err(
            "Config file has changed since LOAD stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }
    
    let merged_output: IntermediateOutput<MergedRecord> = serde_json::from_str(&merged_json)?;
    
    println!("📄 Input: {} merged records", merged_output.record_count);
    println!();
    
    // Validate records
    println!("🔍 Validating records...");
    let validator = Validator::from_flow(&config.flow);
    
    // First filter duplicates
    let (unique_records, duplicate_invalids) =
        validator.filter_duplicates(merged_output.records);
    println!("  ✓ Unique records: {}", unique_records.len());
    println!("  ✗ Duplicates: {}", duplicate_invalids.len());
    
    // Then validate remaining records
    let validation_result = validator.validate(unique_records);
    println!("  ✓ Valid: {}", validation_result.valid.len());
    println!("  ✗ Invalid: {}", validation_result.invalid.len());
    println!();
    
    // Combine all invalid records
    let mut all_invalid = duplicate_invalids;
    all_invalid.extend(validation_result.invalid);
    
    // Save/update invalid records
    if !all_invalid.is_empty() {
        let invalid_path = config.output_config.output_dir.join("invalid_records.csv");
        let writer = CsvWriter::new();
        writer.write_invalid_records(&invalid_path, &all_invalid)?;
        println!("💾 Invalid records saved:");
        println!("  → {}", invalid_path.display());
        println!("  → {} records", all_invalid.len());
        println!();
    }
    
    // Calculate new config hash and wrap output
    let config_hash = calculate_config_hash(config_path)?;
    let output = IntermediateOutput::new(config_hash, validation_result.valid);
    
    // Save validated records
    let output_path = config
        .output_config
        .output_dir
        .join("validated_records.json");
    let json = serde_json::to_string_pretty(&output)?;
    fs::write(&output_path, json)?;
    
    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!("  → {} valid records", output.record_count);
    println!("  → Config hash: {}...", &output.config_hash[..8]);
    println!();
    
    println!("✅ VALIDATE stage complete!");
    println!();
    println!("Next step:");
    println!("  migratus enrich {}", config_path.display());
    
    Ok(())
}
