use crate::domain::config::{AlreadyMigratedConfig, EnrichmentColumns, MigrationConfig};
use crate::domain::records::EnrichedRecord;
use crate::utils::hash::{calculate_config_hash, verify_config_hash};
use crate::utils::intermediate::IntermediateOutput;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const SKIPPED_RECORDS_JSON: &str = "skipped_records.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkippedAlreadyMigratedRecord {
    line_number: crate::domain::types::LineNumber,
    data: std::collections::HashMap<String, String>,
    skip_reason: String,
}

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
    if crate::cli::commands::payment_method_fingerprint_id::is_payment_method_fingerprint_id_config(
        &config,
    ) {
        return crate::cli::commands::payment_method_fingerprint_id::handle_enrich(
            config_path,
            force,
        )
        .await;
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

    let has_string_enrichment = config
        .enrichment
        .as_ref()
        .map(|enrichment| enrichment.string_columns().next().is_some())
        .unwrap_or(false);
    let already_migrated_config = config
        .enrichment
        .as_ref()
        .and_then(|enrichment| enrichment.already_migrated());

    // Check if enrichment is configured
    if !has_string_enrichment && already_migrated_config.is_none() {
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

    // Add all string enrichment columns from config
    if let Some(enrichment) = &config.enrichment {
        for (key, value) in enrichment.string_columns() {
            enrichment_columns.add(key.clone(), value.to_string());
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

    let (enriched_records, skipped_records) =
        filter_already_migrated_records(already_migrated_config, enriched_records)?;
    if !skipped_records.is_empty() {
        write_skipped_records_json_and_csv(&config, config_path, &skipped_records)?;
    }

    println!("  ✓ Enriched: {} records", enriched_records.len());
    if !skipped_records.is_empty() {
        println!("  ✓ Skipped already migrated: {}", skipped_records.len());
    }
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

fn filter_already_migrated_records(
    already_migrated_config: Option<AlreadyMigratedConfig>,
    records: Vec<EnrichedRecord>,
) -> Result<(Vec<EnrichedRecord>, Vec<SkippedAlreadyMigratedRecord>), Box<dyn std::error::Error>> {
    let Some(config) = already_migrated_config else {
        return Ok((records, Vec::new()));
    };

    if config.match_fields.is_empty() {
        return Err("enrichment.already_migrated.match_fields must not be empty".into());
    }

    let match_fields: Vec<String> = config
        .match_fields
        .iter()
        .map(|field| field.to_header_name())
        .collect();
    let already_migrated = read_already_migrated_keys(&config.path, &match_fields)?;
    if already_migrated.is_empty() {
        return Ok((records, Vec::new()));
    }

    let mut kept = Vec::new();
    let mut skipped = Vec::new();

    for record in records {
        let key = record_key(&record.data, &match_fields);
        if key
            .as_ref()
            .map(|key| already_migrated.contains(key))
            .unwrap_or(false)
        {
            skipped.push(SkippedAlreadyMigratedRecord {
                line_number: record.line_number,
                data: record.data,
                skip_reason: "already_migrated".to_string(),
            });
        } else {
            kept.push(record);
        }
    }

    Ok((kept, skipped))
}

fn read_already_migrated_keys(
    path: &Path,
    match_fields: &[String],
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)?;
    let mut keys = HashSet::new();
    let mut field_indexes: Option<Vec<usize>> = None;

    for (row_index, result) in reader.records().enumerate() {
        let record = result?;
        if record.is_empty() {
            continue;
        }

        if row_index == 0 {
            let header_indexes: Option<Vec<usize>> = match_fields
                .iter()
                .map(|field| record.iter().position(|value| value.trim() == field))
                .collect();
            if let Some(indexes) = header_indexes {
                field_indexes = Some(indexes);
                continue;
            }
        }

        let indexes = field_indexes
            .clone()
            .unwrap_or_else(|| (0..match_fields.len()).collect());
        let values: Option<Vec<String>> = indexes
            .iter()
            .map(|index| record.get(*index).map(|value| value.trim().to_string()))
            .collect();

        if let Some(values) = values {
            if values.iter().all(|value| !value.is_empty()) {
                keys.insert(values.join("\u{1f}"));
            }
        }
    }

    Ok(keys)
}

fn record_key(
    data: &std::collections::HashMap<String, String>,
    match_fields: &[String],
) -> Option<String> {
    let values: Option<Vec<String>> = match_fields
        .iter()
        .map(|field| data.get(field).map(|value| value.trim().to_string()))
        .collect();
    values.and_then(|values| {
        if values.iter().all(|value| !value.is_empty()) {
            Some(values.join("\u{1f}"))
        } else {
            None
        }
    })
}

fn write_skipped_records_json_and_csv(
    config: &MigrationConfig,
    config_path: &Path,
    records: &[SkippedAlreadyMigratedRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = IntermediateOutput::new(calculate_config_hash(config_path)?, records.to_vec());
    fs::write(
        config.output_config.output_dir.join(SKIPPED_RECORDS_JSON),
        serde_json::to_string_pretty(&output)?,
    )?;

    let mut writer =
        csv::Writer::from_path(config.output_config.output_dir.join("skipped_records.csv"))?;
    writer.write_record(["line_number", "skip_reason", "data"])?;
    for record in records {
        writer.write_record([
            record.line_number.value().to_string(),
            record.skip_reason.clone(),
            serde_json::to_string(&record.data)?,
        ])?;
    }
    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn reads_already_migrated_keys_from_headered_csv() {
        let dir = std::env::temp_dir().join(format!(
            "migratus-generic-skip-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("already.csv");
        fs::write(&path, "merchant_id,payment_method_id\nm1,pm_1\nm2,pm_2\n").unwrap();

        let keys = read_already_migrated_keys(
            &path,
            &["merchant_id".to_string(), "payment_method_id".to_string()],
        )
        .unwrap();

        assert!(keys.contains("m1\u{1f}pm_1"));
        assert!(keys.contains("m2\u{1f}pm_2"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn filters_records_by_already_migrated_config() {
        let dir = std::env::temp_dir().join(format!(
            "migratus-generic-filter-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("already.csv");
        fs::write(&path, "payment_method_id\npm_2\n").unwrap();

        let mut first = HashMap::new();
        first.insert("payment_method_id".to_string(), "pm_1".to_string());
        let mut second = HashMap::new();
        second.insert("payment_method_id".to_string(), "pm_2".to_string());

        let config = AlreadyMigratedConfig {
            path,
            match_fields: vec![crate::domain::migration_field::MigrationField::PaymentMethodId],
        };
        let (kept, skipped) = filter_already_migrated_records(
            Some(config),
            vec![
                EnrichedRecord::new(crate::domain::types::LineNumber::new(2), first),
                EnrichedRecord::new(crate::domain::types::LineNumber::new(3), second),
            ],
        )
        .unwrap();

        assert_eq!(kept.len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].line_number.value(), 3);
        fs::remove_dir_all(dir).unwrap();
    }
}
