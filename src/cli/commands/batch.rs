use crate::domain::config::MigrationConfig;
use crate::domain::records::EnrichedRecord;
use crate::domain::types::BatchNumber;
use crate::utils::hash::verify_config_hash;
use crate::utils::intermediate::IntermediateOutput;
use std::fs;
use std::path::Path;

pub async fn handle_batch(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 BATCH Stage");
    println!("==============\n");

    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;

    if crate::cli::commands::customer_global_id::is_customer_global_id_config(&config) {
        return crate::cli::commands::customer_global_id::handle_batch(config_path, force).await;
    }

    // Read enriched records
    let enriched_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    if !enriched_path.exists() {
        return Err(format!(
            "Enriched records not found. Run 'migratus enrich {}' first",
            config_path.display()
        )
        .into());
    }

    let enriched_json = fs::read_to_string(&enriched_path)?;

    // Verify config hash
    if !force && !verify_config_hash(&enriched_json, config_path)? {
        return Err(
            "Config file has changed since ENRICH stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }

    let enriched_output: IntermediateOutput<EnrichedRecord> = serde_json::from_str(&enriched_json)?;

    println!(
        "📄 Input: {} enriched records",
        enriched_output.record_count
    );
    println!();

    // Create batches directory
    let batches_dir = config.output_config.output_dir.join("batches");
    fs::create_dir_all(&batches_dir)?;

    // Split into batches
    println!("✂️  Creating batches...");
    let batch_size = config.batch_config.batch_size;
    let records = enriched_output.records;

    let mut batch_count = 0;

    for (i, chunk) in records.chunks(batch_size).enumerate() {
        let batch_number = BatchNumber::new(i + 1);
        let batch_file = batches_dir.join(format!("batch_{:04}.csv", batch_number.value()));

        // Write batch as CSV
        let mut wtr = csv::Writer::from_path(&batch_file)?;

        // Write headers (from first record)
        if let Some(first_record) = chunk.first() {
            let headers: Vec<String> = first_record.data.keys().cloned().collect();
            wtr.write_record(&headers)?;

            // Write all records in this batch
            for record in chunk {
                let values: Vec<String> = headers
                    .iter()
                    .map(|h| record.data.get(h).cloned().unwrap_or_default())
                    .collect();
                wtr.write_record(&values)?;
            }
        }

        wtr.flush()?;
        batch_count += 1;
    }

    println!("  ✓ Created {} batches", batch_count);
    println!("  ✓ Batch size: {} records", batch_size);
    println!("  ✓ Location: {}", batches_dir.display());
    println!();

    println!("💾 Output saved:");
    println!("  → {}/batch_*.csv", batches_dir.display());
    println!("  → {} batch files", batch_count);
    println!();

    println!("✅ BATCH stage complete!");
    println!();
    println!("Next step:");
    println!("  migratus migrate {}", config_path.display());
    println!(
        "  (or: migratus migrate {} --count 10)",
        config_path.display()
    );

    Ok(())
}
