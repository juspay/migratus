use crate::domain::config::{EnrichmentColumns, MigrationConfig};
use crate::domain::records::EnrichedRecord;
use crate::utils::hash::{calculate_config_hash, verify_config_hash};
use crate::utils::intermediate::IntermediateOutput;
use std::fs;
use std::path::Path;

pub async fn handle_enrich(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("➕ ENRICH Stage");
    println!("===============\n");

    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;

    if crate::cli::commands::customer_global_id::is_customer_global_id_config(&config) {
        return crate::cli::commands::customer_global_id::handle_enrich(config_path, force).await;
    }

    // Read validated records
    let validated_path = config
        .output_config
        .output_dir
        .join("validated_records.json");
    if !validated_path.exists() {
        return Err(format!(
            "Validated records not found. Run 'migratus validate {}' first",
            config_path.display()
        )
        .into());
    }

    let validated_json = fs::read_to_string(&validated_path)?;

    // Verify config hash
    if !force && !verify_config_hash(&validated_json, config_path)? {
        return Err(
            "Config file has changed since VALIDATE stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }

    let validated_output: IntermediateOutput<EnrichedRecord> =
        serde_json::from_str(&validated_json)?;

    println!(
        "📄 Input: {} validated records",
        validated_output.record_count
    );
    println!();

    // Check if enrichment is configured
    if config.enrichment.is_none() {
        println!("ℹ️  No enrichment configured, skipping enrichment stage");
        println!("  → Copying validated records to enriched records");
        println!();

        // Just copy validated to enriched
        let config_hash = calculate_config_hash(config_path)?;
        let output = IntermediateOutput::new(config_hash, validated_output.records);

        let output_path = config
            .output_config
            .output_dir
            .join("enriched_records.json");
        let json = serde_json::to_string_pretty(&output)?;
        fs::write(&output_path, json)?;

        println!("💾 Output saved:");
        println!("  → {}", output_path.display());
        println!(
            "  → {} records (no enrichment applied)",
            output.record_count
        );
        println!();

        println!("✅ ENRICH stage complete (skipped)!");
        println!();
        println!("Next step:");
        println!("  migratus batch {}", config_path.display());

        return Ok(());
    }

    // Enrich records with configured columns
    println!("🔧 Enriching records...");
    let mut enrichment_columns = EnrichmentColumns::new();

    // Add all enrichment columns from config
    if let Some(enrichment) = &config.enrichment {
        for (key, value) in enrichment {
            enrichment_columns.add(key.clone(), value.clone());
        }
    }

    let enriched_records: Vec<EnrichedRecord> = validated_output
        .records
        .into_iter()
        .map(|mut record| {
            // Add enrichment columns
            for (key, value) in &enrichment_columns.columns {
                record.data.insert(key.clone(), value.clone());
            }
            record
        })
        .collect();

    println!("  ✓ Enriched: {} records", enriched_records.len());
    if !enrichment_columns.columns.is_empty() {
        println!("  ✓ Added columns:");
        for key in enrichment_columns.columns.keys() {
            println!("    - {}", key);
        }
    }
    println!();

    // Calculate new config hash and wrap output
    let config_hash = calculate_config_hash(config_path)?;
    let output = IntermediateOutput::new(config_hash, enriched_records);

    // Save enriched records
    let output_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    let json = serde_json::to_string_pretty(&output)?;
    fs::write(&output_path, json)?;

    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!("  → {} enriched records", output.record_count);
    println!("  → Config hash: {}...", &output.config_hash[..8]);
    println!();

    println!("✅ ENRICH stage complete!");
    println!();
    println!("Next step:");
    println!("  migratus batch {}", config_path.display());

    Ok(())
}
