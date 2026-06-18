use crate::domain::config::{DataSource, MigrationConfig};
use crate::domain::customer_global_id::{
    loaded_record_from_csv, records_to_csv_bytes, split_records_into_batches, validate_headers,
    validate_loaded_record, CustomerGlobalIdApiResponse, CustomerGlobalIdBatchFile,
    CustomerGlobalIdJsonlResult, CustomerGlobalIdLoadedRecord, CustomerGlobalIdMigrationRecord,
    CustomerGlobalIdMigrationSummary, CustomerGlobalIdStatus, InvalidCustomerGlobalIdRecord,
    SavedCustomerGlobalIdBatchResponse,
};
use crate::operations::api::CustomerGlobalIdApiClient;
use crate::utils::hash::{calculate_config_hash, verify_config_hash};
use crate::utils::intermediate::IntermediateOutput;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const INVALID_RECORDS_JSON: &str = "invalid_records.json";

#[derive(Debug, Clone)]
struct CustomerGlobalIdFailedOutputRecord {
    line_number: Option<i64>,
    batch_number: usize,
    batch_file: String,
    merchant_id: Option<crate::domain::types::MerchantId>,
    customer_id: Option<crate::domain::types::CustomerId>,
    status: CustomerGlobalIdStatus,
    error: Option<String>,
}

pub fn is_customer_global_id_config(config: &MigrationConfig) -> bool {
    config.flow.is_customer_global_id()
}

pub async fn handle_load(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("📥 LOAD Stage - Customer Global ID Migration");
    println!("============================================\n");

    let config_hash = calculate_config_hash(config_path)?;
    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;
    fs::create_dir_all(&config.output_config.output_dir)?;

    let input_path = match &config.data_source {
        DataSource::Merged { path } => path,
        DataSource::Separate { .. } => {
            return Err("customer_global_id flow requires a merged CSV data source".into())
        }
    };

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(input_path)?;
    let headers = reader.headers()?.clone();
    let header_index = validate_headers(&headers)?;

    let mut records = Vec::new();
    for (index, result) in reader.records().enumerate() {
        let record = result?;
        records.push(loaded_record_from_csv(
            &headers,
            header_index,
            index + 2,
            record,
        ));
    }

    let output = IntermediateOutput::new(config_hash, records);
    let output_path = config.output_config.output_dir.join("merged_records.json");
    fs::write(&output_path, serde_json::to_string_pretty(&output)?)?;

    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!("  → {} records", output.record_count);
    println!();
    println!("✅ LOAD stage complete!");
    println!("Next step:");
    println!("  migratus validate {}", config_path.display());

    Ok(())
}

pub async fn handle_validate(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ VALIDATE Stage - Customer Global ID Migration");
    println!("================================================\n");

    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;

    let merged_path = config.output_config.output_dir.join("merged_records.json");
    if !merged_path.exists() {
        return Err(format!(
            "Merged records not found. Run 'migratus load {}' first",
            config_path.display()
        )
        .into());
    }

    let merged_json = fs::read_to_string(&merged_path)?;
    if !force && !verify_config_hash(&merged_json, config_path)? {
        return Err(
            "Config file has changed since LOAD stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }

    let merged_output: IntermediateOutput<CustomerGlobalIdLoadedRecord> =
        serde_json::from_str(&merged_json)?;

    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for record in merged_output.records {
        match validate_loaded_record(record) {
            Ok(record) => valid.push(record),
            Err(record) => invalid.push(record),
        }
    }

    write_invalid_records_json_and_csv(&config, config_path, &invalid)?;

    let output = IntermediateOutput::new(calculate_config_hash(config_path)?, valid);
    let output_path = config
        .output_config
        .output_dir
        .join("validated_records.json");
    fs::write(&output_path, serde_json::to_string_pretty(&output)?)?;

    println!("📊 Validation Results:");
    println!("  ✓ Valid: {}", output.record_count);
    println!("  ✗ Invalid: {}", invalid.len());
    println!();
    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!();
    println!("✅ VALIDATE stage complete!");
    println!("Next step:");
    println!("  migratus enrich {}", config_path.display());

    Ok(())
}

pub async fn handle_enrich(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("➕ ENRICH Stage - Customer Global ID Migration");
    println!("=============================================\n");

    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;

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
    if !force && !verify_config_hash(&validated_json, config_path)? {
        return Err(
            "Config file has changed since VALIDATE stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }

    let validated_output: IntermediateOutput<CustomerGlobalIdMigrationRecord> =
        serde_json::from_str(&validated_json)?;
    let output = IntermediateOutput::new(
        calculate_config_hash(config_path)?,
        validated_output.records,
    );
    let output_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    fs::write(&output_path, serde_json::to_string_pretty(&output)?)?;

    println!("ℹ️  No enrichment required for customer global ID migration");
    println!("💾 Output saved:");
    println!("  → {}", output_path.display());
    println!("  → {} records", output.record_count);
    println!();
    println!("✅ ENRICH stage complete!");
    println!("Next step:");
    println!("  migratus batch {}", config_path.display());

    Ok(())
}

pub async fn handle_batch(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 BATCH Stage - Customer Global ID Migration");
    println!("============================================\n");

    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;

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
    if !force && !verify_config_hash(&enriched_json, config_path)? {
        return Err(
            "Config file has changed since ENRICH stage. Use --force to override or re-run from LOAD"
                .into(),
        );
    }

    let enriched_output: IntermediateOutput<CustomerGlobalIdMigrationRecord> =
        serde_json::from_str(&enriched_json)?;

    let (batches, batching_invalid) = split_records_into_batches(
        enriched_output.records,
        config.batch_config.batch_size,
        config.batch_config.max_file_size_bytes,
    )?;

    let batches_dir = config.output_config.output_dir.join("batches");
    fs::create_dir_all(&batches_dir)?;

    for batch in &batches {
        let batch_path = batches_dir.join(&batch.file_name);
        fs::write(&batch_path, records_to_csv_bytes(&batch.records)?)?;
    }

    if !batching_invalid.is_empty() {
        let mut invalid = read_invalid_records_json(&config.output_config.output_dir)?;
        invalid.extend(batching_invalid);
        write_invalid_records_json_and_csv(&config, config_path, &invalid)?;
    }

    println!("📊 Batching Results:");
    println!("  ✓ Batch files: {}", batches.len());
    println!("  ✓ Batch size limit: {}", config.batch_config.batch_size);
    println!(
        "  ✓ File size limit: {} bytes",
        config.batch_config.max_file_size_bytes
    );
    println!("  → {}", batches_dir.display());
    println!();
    println!("✅ BATCH stage complete!");
    println!("Next step:");
    println!("  migratus migrate {}", config_path.display());

    Ok(())
}

pub async fn handle_migrate(
    config_path: &Path,
    from_batch: Option<usize>,
    count: usize,
    all: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 MIGRATE Stage - Customer Global ID Migration");
    println!("===============================================\n");

    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;

    let enriched_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    if enriched_path.exists() {
        let enriched_json = fs::read_to_string(&enriched_path)?;
        if !force && !verify_config_hash(&enriched_json, config_path)? {
            return Err(
                "Config file has changed since ENRICH stage. Use --force to override or re-run from LOAD"
                    .into(),
            );
        }
    }

    let batches_dir = config.output_config.output_dir.join("batches");
    let batch_files = list_batch_files(&batches_dir)?;
    if batch_files.is_empty() {
        return Err(format!(
            "Batches not found. Run 'migratus batch {}' first",
            config_path.display()
        )
        .into());
    }

    let batch_response_dir = config.output_config.batch_response_dir();
    fs::create_dir_all(batch_response_dir.path())?;

    let total_batches = batch_files.len();
    let completed = completed_batches(batch_response_dir.path())?;
    let start_batch = from_batch.unwrap_or_else(|| first_missing_batch(total_batches, &completed));
    let end_batch = if all {
        total_batches
    } else {
        std::cmp::min(start_batch + count - 1, total_batches)
    };

    if start_batch > total_batches {
        println!("✅ All batches already migrated!");
        return Ok(());
    }

    let jobs: Vec<_> = batch_files
        .into_iter()
        .filter(|batch| batch.batch_number >= start_batch && batch.batch_number <= end_batch)
        .filter(|batch| {
            force
                || !batch_response_dir
                    .path()
                    .join(format!("batch_{:04}.json", batch.batch_number))
                    .exists()
        })
        .collect();

    println!("🎯 Migration Plan:");
    println!("  Start batch: {}", start_batch);
    println!("  End batch: {}", end_batch);
    println!("  Batches to upload: {}", jobs.len());
    println!(
        "  Parallel uploads: {}",
        config.batch_config.parallel_uploads
    );
    println!();

    if jobs.is_empty() {
        println!("✅ Selected batches already have responses.");
        return Ok(());
    }

    let client = CustomerGlobalIdApiClient::new(
        config.api_config.endpoint.clone(),
        config.api_config.api_key.clone(),
        config.api_config.timeout(),
        config.batch_config.retry_count,
        Duration::from_millis(config.batch_config.retry_backoff_ms),
    )?;

    let parallel_uploads = std::cmp::max(1, config.batch_config.parallel_uploads);
    let semaphore = Arc::new(Semaphore::new(parallel_uploads));
    let response_dir = batch_response_dir.path().to_path_buf();
    let mut join_set = JoinSet::new();

    for job in jobs {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = client.clone();
        let response_dir = response_dir.clone();
        join_set.spawn(async move {
            let _permit = permit;
            upload_and_save_batch(client, response_dir, job).await
        });
    }

    let mut saved = 0usize;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {
                saved += 1;
            }
            Ok(Err(error)) => {
                join_set.abort_all();
                return Err(error.to_string().into());
            }
            Err(error) => {
                join_set.abort_all();
                return Err(error.to_string().into());
            }
        }
    }

    println!("📊 Migration Results:");
    println!("  ✓ Response files saved: {}", saved);
    println!();
    println!("✅ MIGRATE stage complete!");
    println!("Next step:");
    if end_batch < total_batches {
        if all {
            println!(
                "  migratus migrate {} --from-batch {} --all",
                config_path.display(),
                end_batch + 1
            );
        } else {
            println!(
                "  migratus migrate {} --from-batch {} --count {}",
                config_path.display(),
                end_batch + 1,
                count
            );
        }
    } else {
        println!("  migratus complete {}", config_path.display());
    }

    Ok(())
}

pub async fn handle_complete(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("📝 COMPLETE Stage - Customer Global ID Migration");
    println!("================================================\n");

    let config = load_config(config_path)?;
    ensure_customer_flow(&config)?;

    let response_dir = config.output_config.batch_response_dir();
    if !response_dir.path().exists() {
        return Err(format!(
            "Batch responses not found. Run 'migratus migrate {}' first",
            config_path.display()
        )
        .into());
    }

    let merged_count =
        read_record_count(&config.output_config.output_dir.join("merged_records.json"))?;
    let valid_count = read_record_count(
        &config
            .output_config
            .output_dir
            .join("validated_records.json"),
    )?;
    let invalid_records = read_invalid_records_json(&config.output_config.output_dir)?;
    let total_batches = list_batch_files(&config.output_config.output_dir.join("batches"))?.len();
    let batch_records_by_number = load_batch_records_by_number(&config)?;

    let mut per_status_counts: BTreeMap<CustomerGlobalIdStatus, usize> =
        CustomerGlobalIdStatus::known_api_statuses()
            .iter()
            .map(|status| (status.clone(), 0))
            .collect();
    let mut total_updated_count = 0usize;
    let mut total_skipped_count = 0usize;
    let mut total_failed_count = 0usize;
    let mut jsonl_lines = Vec::new();
    let mut failed_records = Vec::new();

    for response_path in list_response_files(response_dir.path())? {
        let response_json = fs::read_to_string(&response_path)?;
        let saved: SavedCustomerGlobalIdBatchResponse = serde_json::from_str(&response_json)?;

        if let Some(error) = &saved.transport_error {
            *per_status_counts
                .entry(CustomerGlobalIdStatus::TransportError)
                .or_default() += saved.record_count;
            total_failed_count += saved.record_count;
            failed_records.push(CustomerGlobalIdFailedOutputRecord {
                line_number: None,
                batch_number: saved.batch_number,
                batch_file: saved.batch_file.clone(),
                merchant_id: None,
                customer_id: None,
                status: CustomerGlobalIdStatus::TransportError,
                error: Some(error.clone()),
            });
            jsonl_lines.push(serde_json::to_string(&CustomerGlobalIdJsonlResult {
                batch_number: saved.batch_number,
                batch_file: saved.batch_file.clone(),
                merchant_id: None,
                customer_id: None,
                status: CustomerGlobalIdStatus::TransportError,
                error: Some(error.clone()),
            })?);
            continue;
        }

        let parsed = parse_customer_response_body(&saved.body)?;
        if parsed.results.is_empty() {
            total_updated_count += parsed.updated_count;
            total_skipped_count += parsed.skipped_count;
            total_failed_count += parsed.failed_count;
            continue;
        }

        for (result_index, result) in parsed.results.into_iter().enumerate() {
            *per_status_counts.entry(result.status.clone()).or_default() += 1;
            let is_failed = result.status.is_failed();
            if result.status.is_updated() {
                total_updated_count += 1;
            } else if is_failed {
                total_failed_count += 1;
            } else {
                total_skipped_count += 1;
            }

            if is_failed {
                let line_number = infer_result_line_number(
                    &batch_records_by_number,
                    saved.batch_number,
                    result_index,
                    result.merchant_id.as_ref(),
                    result.customer_id.as_ref(),
                )
                .or(result.line_number);
                failed_records.push(CustomerGlobalIdFailedOutputRecord {
                    line_number,
                    batch_number: saved.batch_number,
                    batch_file: saved.batch_file.clone(),
                    merchant_id: result.merchant_id.clone(),
                    customer_id: result.customer_id.clone(),
                    status: result.status.clone(),
                    error: result.error.clone(),
                });
            }

            jsonl_lines.push(serde_json::to_string(&CustomerGlobalIdJsonlResult {
                batch_number: saved.batch_number,
                batch_file: saved.batch_file.clone(),
                merchant_id: result.merchant_id,
                customer_id: result.customer_id,
                status: result.status,
                error: result.error,
            })?);
        }
    }

    let summary = CustomerGlobalIdMigrationSummary {
        total_input_rows: merged_count,
        valid_rows: valid_count,
        invalid_input_rows: invalid_records.len(),
        total_batches,
        total_updated_count,
        total_skipped_count,
        total_failed_count,
        per_status_counts,
    };

    let summary_json = serde_json::to_string_pretty(&summary)?;
    fs::write(
        config
            .output_config
            .output_dir
            .join("migration_summary.json"),
        &summary_json,
    )?;
    fs::write(
        config.output_config.output_dir.join("summary.json"),
        summary_json,
    )?;
    fs::write(
        config
            .output_config
            .output_dir
            .join("migration_results.jsonl"),
        jsonl_lines.join("\n"),
    )?;
    write_invalid_records_csv(
        &config.output_config.output_dir.join("invalid_records.csv"),
        &invalid_records,
    )?;
    write_failed_records_csv(
        &config.output_config.output_dir.join("failed_records.csv"),
        &failed_records,
    )?;

    println!("📊 Final Summary:");
    println!("  Total input rows: {}", summary.total_input_rows);
    println!("  ✓ Valid rows: {}", summary.valid_rows);
    println!("  ✗ Invalid input rows: {}", summary.invalid_input_rows);
    println!("  Total batches: {}", summary.total_batches);
    println!("  Updated: {}", summary.total_updated_count);
    println!("  Skipped: {}", summary.total_skipped_count);
    println!("  Failed: {}", summary.total_failed_count);
    println!();
    println!("✅ COMPLETE stage finished!");

    Ok(())
}

fn load_config(config_path: &Path) -> Result<MigrationConfig, Box<dyn std::error::Error>> {
    let config_json = fs::read_to_string(config_path)?;
    Ok(serde_json::from_str(&config_json)?)
}

fn ensure_customer_flow(config: &MigrationConfig) -> Result<(), Box<dyn std::error::Error>> {
    if is_customer_global_id_config(config) {
        Ok(())
    } else {
        Err("Config flow is not customer_global_id".into())
    }
}

fn write_invalid_records_json_and_csv(
    config: &MigrationConfig,
    config_path: &Path,
    records: &[InvalidCustomerGlobalIdRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = IntermediateOutput::new(calculate_config_hash(config_path)?, records.to_vec());
    fs::write(
        config.output_config.output_dir.join(INVALID_RECORDS_JSON),
        serde_json::to_string_pretty(&output)?,
    )?;
    write_invalid_records_csv(
        &config.output_config.output_dir.join("invalid_records.csv"),
        records,
    )?;
    Ok(())
}

fn write_invalid_records_csv(
    path: &Path,
    records: &[InvalidCustomerGlobalIdRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(["line_number", "invalid_reason", "failed_at_stage", "data"])?;
    for record in records {
        writer.write_record([
            record.line_number.value().to_string(),
            record.invalid_reason.to_string(),
            record.failed_at_stage.to_string(),
            serde_json::to_string(&record.original_data)?,
        ])?;
    }
    writer.flush()?;
    Ok(())
}

fn write_failed_records_csv(
    path: &Path,
    records: &[CustomerGlobalIdFailedOutputRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "batch_number",
        "batch_file",
        "line_number",
        "merchant_id",
        "customer_id",
        "status",
        "error",
    ])?;

    for record in records {
        writer.write_record([
            record.batch_number.to_string(),
            record.batch_file.clone(),
            record
                .line_number
                .map(|line_number| line_number.to_string())
                .unwrap_or_default(),
            record
                .merchant_id
                .as_ref()
                .map(|merchant_id| merchant_id.inner().to_string())
                .unwrap_or_default(),
            record
                .customer_id
                .as_ref()
                .map(|customer_id| customer_id.inner().to_string())
                .unwrap_or_default(),
            record.status.to_string(),
            record.error.clone().unwrap_or_default(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

fn read_invalid_records_json(
    output_dir: &Path,
) -> Result<Vec<InvalidCustomerGlobalIdRecord>, Box<dyn std::error::Error>> {
    let path = output_dir.join(INVALID_RECORDS_JSON);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(path)?;
    let output: IntermediateOutput<InvalidCustomerGlobalIdRecord> = serde_json::from_str(&json)?;
    Ok(output.records)
}

fn load_batch_records_by_number(
    config: &MigrationConfig,
) -> Result<BTreeMap<usize, Vec<CustomerGlobalIdMigrationRecord>>, Box<dyn std::error::Error>> {
    let enriched_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    if !enriched_path.exists() {
        return Ok(BTreeMap::new());
    }

    let enriched_json = fs::read_to_string(&enriched_path)?;
    let enriched_output: IntermediateOutput<CustomerGlobalIdMigrationRecord> =
        serde_json::from_str(&enriched_json)?;
    let (batches, _invalid) = split_records_into_batches(
        enriched_output.records,
        config.batch_config.batch_size,
        config.batch_config.max_file_size_bytes,
    )?;

    Ok(batches
        .into_iter()
        .map(|batch| (batch.batch_number.value(), batch.records))
        .collect())
}

fn infer_result_line_number(
    batch_records_by_number: &BTreeMap<usize, Vec<CustomerGlobalIdMigrationRecord>>,
    batch_number: usize,
    result_index: usize,
    merchant_id: Option<&crate::domain::types::MerchantId>,
    customer_id: Option<&crate::domain::types::CustomerId>,
) -> Option<i64> {
    let batch_records = batch_records_by_number.get(&batch_number)?;

    if let Some(record) = batch_records.get(result_index) {
        return Some(record.line_number.value() as i64);
    }

    match (merchant_id, customer_id) {
        (Some(merchant_id), Some(customer_id)) => batch_records
            .iter()
            .find(|record| &record.merchant_id == merchant_id && &record.customer_id == customer_id)
            .map(|record| record.line_number.value() as i64),
        (Some(merchant_id), None) => batch_records
            .iter()
            .find(|record| &record.merchant_id == merchant_id)
            .map(|record| record.line_number.value() as i64),
        (None, Some(customer_id)) => batch_records
            .iter()
            .find(|record| &record.customer_id == customer_id)
            .map(|record| record.line_number.value() as i64),
        (None, None) => None,
    }
}

fn list_batch_files(
    dir: &Path,
) -> Result<Vec<CustomerGlobalIdBatchFile>, Box<dyn std::error::Error>> {
    let mut batches = Vec::new();
    if !dir.exists() {
        return Ok(batches);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with("batch_") || !file_name.ends_with(".csv") {
            continue;
        }
        let batch_number = parse_batch_number(&file_name, ".csv")?;
        batches.push(CustomerGlobalIdBatchFile {
            batch_number,
            record_count: count_csv_data_rows(&path)?,
            byte_size: fs::metadata(&path)?.len() as usize,
            path,
        });
    }

    batches.sort_by_key(|batch| batch.batch_number);
    Ok(batches)
}

fn completed_batches(dir: &Path) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    Ok(list_response_files(dir)?
        .iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| parse_batch_number(name, ".json").ok())
        })
        .collect())
}

fn first_missing_batch(total_batches: usize, completed: &[usize]) -> usize {
    for batch_number in 1..=total_batches {
        if !completed.contains(&batch_number) {
            return batch_number;
        }
    }
    total_batches + 1
}

fn list_response_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("batch_") && name.ends_with(".json"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn parse_batch_number(
    file_name: &str,
    extension: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let number = file_name
        .strip_prefix("batch_")
        .and_then(|value| value.strip_suffix(extension))
        .ok_or_else(|| format!("Invalid batch filename: {}", file_name))?;
    Ok(number.parse()?)
}

fn count_csv_data_rows(path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    Ok(reader.records().count())
}

async fn upload_and_save_batch(
    client: CustomerGlobalIdApiClient,
    response_dir: PathBuf,
    job: CustomerGlobalIdBatchFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_name = job
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Batch file name is not valid UTF-8")?
        .to_string();
    let started_at = chrono::Utc::now().to_rfc3339();
    let outcome = client.upload_batch_file(&job.path, &file_name).await;
    let completed_at = chrono::Utc::now().to_rfc3339();

    let saved = SavedCustomerGlobalIdBatchResponse {
        batch_number: job.batch_number,
        batch_file: file_name,
        record_count: job.record_count,
        byte_size: job.byte_size,
        endpoint: client.endpoint().to_string(),
        started_at,
        completed_at,
        attempts: outcome.attempts,
        http_status: outcome.http_status,
        headers: outcome.headers,
        body: outcome.body,
        transport_error: outcome.transport_error,
    };

    let response_path = response_dir.join(format!("batch_{:04}.json", job.batch_number));
    tokio::fs::write(response_path, serde_json::to_string_pretty(&saved)?).await?;

    if let Some(error) = &saved.transport_error {
        return Err(format!(
            "Batch {} failed with transport error after {} attempts: {}",
            saved.batch_number, saved.attempts, error
        )
        .into());
    }

    if let Some(status) = saved.http_status {
        if !(200..300).contains(&status) {
            return Err(format!(
                "Batch {} failed with HTTP status {}. Response saved to batch_responses/batch_{:04}.json",
                saved.batch_number, status, saved.batch_number
            )
            .into());
        }
    }

    Ok(())
}

fn parse_customer_response_body(
    body: &serde_json::Value,
) -> Result<CustomerGlobalIdApiResponse, Box<dyn std::error::Error>> {
    if let Some(results) = body.get("row_results") {
        let mut copy = body.clone();
        if let Some(object) = copy.as_object_mut() {
            object.insert("results".to_string(), results.clone());
        }
        return Ok(serde_json::from_value(copy)?);
    }
    Ok(serde_json::from_value(body.clone())?)
}

fn read_record_count(path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(0);
    }
    let json = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    Ok(value
        .get("record_count")
        .and_then(|count| count.as_u64())
        .unwrap_or(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_response_body_with_known_statuses() {
        let body = serde_json::json!({
            "updated_count": 2,
            "skipped_count": 1,
            "failed_count": 1,
            "results": [
                {"merchant_id": "m1", "customer_id": "c1", "status": "updated_null_id"},
                {"merchant_id": "m1", "customer_id": "c2", "status": "updated_non_global_id"},
                {"merchant_id": "m1", "customer_id": "c3", "status": "already_global_id"},
                {"merchant_id": "m1", "customer_id": "c4", "status": "update_failed", "error": "db"}
            ]
        });

        let parsed = parse_customer_response_body(&body).unwrap();
        assert_eq!(parsed.results.len(), 4);
        assert_eq!(parsed.updated_count, 2);
        assert_eq!(parsed.failed_count, 1);
    }

    #[test]
    fn writes_failed_records_csv() {
        let path = std::env::temp_dir().join(format!(
            "migratus-failed-records-{}.csv",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));

        let records = vec![CustomerGlobalIdFailedOutputRecord {
            line_number: Some(3),
            batch_number: 7,
            batch_file: "batch_0007.csv".to_string(),
            merchant_id: Some(crate::domain::types::MerchantId::new("m1".to_string())),
            customer_id: Some(crate::domain::types::CustomerId::new("c1".to_string())),
            status: CustomerGlobalIdStatus::UpdateFailed,
            error: Some("db error".to_string()),
        }];

        write_failed_records_csv(&path, &records).unwrap();
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv
            .contains("batch_number,batch_file,line_number,merchant_id,customer_id,status,error"));
        assert!(csv.contains("7,batch_0007.csv,3,m1,c1,update_failed,db error"));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn infers_missing_api_line_number_from_batch_order() {
        let mut batch_records_by_number = BTreeMap::new();
        batch_records_by_number.insert(
            4,
            vec![
                CustomerGlobalIdMigrationRecord {
                    line_number: crate::domain::types::LineNumber::new(17),
                    merchant_id: crate::domain::types::MerchantId::new("m1".to_string()),
                    customer_id: crate::domain::types::CustomerId::new("c1".to_string()),
                    original_data: Default::default(),
                },
                CustomerGlobalIdMigrationRecord {
                    line_number: crate::domain::types::LineNumber::new(18),
                    merchant_id: crate::domain::types::MerchantId::new("m1".to_string()),
                    customer_id: crate::domain::types::CustomerId::new("c2".to_string()),
                    original_data: Default::default(),
                },
            ],
        );

        let line_number = infer_result_line_number(&batch_records_by_number, 4, 1, None, None);
        assert_eq!(line_number, Some(18));
    }

    #[test]
    fn parses_backend_row_number_alias() {
        let body = serde_json::json!({
            "failed_count": 1,
            "results": [
                {
                    "customer_id": null,
                    "merchant_id": "df198",
                    "message": "CSV row must contain valid merchant_id and customer_id values",
                    "row_number": 2,
                    "status": "invalid_csv_row"
                }
            ]
        });

        let parsed = parse_customer_response_body(&body).unwrap();
        assert_eq!(parsed.results[0].line_number, Some(2));
        assert_eq!(
            parsed.results[0].status,
            CustomerGlobalIdStatus::InvalidCsvRow
        );
    }
}
