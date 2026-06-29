use crate::domain::config::{AlreadyMigratedConfig, DataSource, MigrationConfig};
use crate::domain::migration_field::MigrationField;
use crate::domain::payment_method_fingerprint_id::{
    loaded_record_from_csv, records_to_csv_bytes, split_records_into_batches, validate_headers,
    validate_loaded_record, InvalidPaymentMethodFingerprintIdRecord,
    PaymentMethodFingerprintIdApiResponse, PaymentMethodFingerprintIdApiRowResult,
    PaymentMethodFingerprintIdBatchFile, PaymentMethodFingerprintIdInvalidReason,
    PaymentMethodFingerprintIdInvalidStage, PaymentMethodFingerprintIdJsonlResult,
    PaymentMethodFingerprintIdLoadedRecord, PaymentMethodFingerprintIdMigrationRecord,
    PaymentMethodFingerprintIdMigrationSummary, PaymentMethodFingerprintIdSkipReason,
    PaymentMethodFingerprintIdStatus, SavedPaymentMethodFingerprintIdBatchResponse,
    SkippedPaymentMethodFingerprintIdRecord,
};
use crate::operations::api::PaymentMethodFingerprintIdApiClient;
use crate::utils::hash::{calculate_config_hash, verify_config_hash};
use crate::utils::intermediate::IntermediateOutput;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const INVALID_RECORDS_JSON: &str = "invalid_records.json";
const ALREADY_MIGRATED_RECORDS_JSON: &str = "already_migrated_records.json";
const ALREADY_MIGRATED_PAYMENT_METHODS_PATH_KEY: &str = "already_migrated_payment_methods_path";

#[derive(Debug, Clone)]
struct FingerprintSuccessfulOutputRecord {
    row_number: Option<i64>,
    batch_number: usize,
    batch_file: String,
    merchant_id: Option<crate::domain::types::MerchantId>,
    payment_method_id: Option<crate::domain::types::PaymentMethodId>,
    old_fingerprint_id: Option<String>,
    new_fingerprint_id: Option<String>,
    migration_status: PaymentMethodFingerprintIdStatus,
}

#[derive(Debug, Clone)]
struct FingerprintFailedOutputRecord {
    row_number: Option<i64>,
    batch_number: usize,
    batch_file: String,
    merchant_id: Option<crate::domain::types::MerchantId>,
    payment_method_id: Option<crate::domain::types::PaymentMethodId>,
    old_fingerprint_id: Option<String>,
    new_fingerprint_id: Option<String>,
    migration_status: PaymentMethodFingerprintIdStatus,
    error: Option<String>,
}

pub fn is_payment_method_fingerprint_id_config(config: &MigrationConfig) -> bool {
    config.flow.is_payment_method_fingerprint_id()
}

pub async fn handle_load(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("LOAD Stage - Payment Method Fingerprint ID Migration");
    println!("====================================================\n");

    let config_hash = calculate_config_hash(config_path)?;
    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;
    fs::create_dir_all(&config.output_config.output_dir)?;

    let input_path = match &config.data_source {
        DataSource::Merged { path } => path,
        DataSource::Separate { .. } => {
            return Err("payment_method_fingerprint flow requires a merged CSV data source".into())
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

    println!("Output saved:");
    println!("  -> {}", output_path.display());
    println!("  -> {} records", output.record_count);
    println!();
    println!("LOAD stage complete");
    println!("Next step:");
    println!("  migratus validate {}", config_path.display());

    Ok(())
}

pub async fn handle_validate(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("VALIDATE Stage - Payment Method Fingerprint ID Migration");
    println!("========================================================\n");

    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;

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

    let merged_output: IntermediateOutput<PaymentMethodFingerprintIdLoadedRecord> =
        serde_json::from_str(&merged_json)?;

    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();

    for record in merged_output.records {
        match validate_loaded_record(record) {
            Ok(record) => {
                let key = (
                    record.merchant_id.inner().to_string(),
                    record.payment_method_id.inner().to_string(),
                );
                if let Some(first_line_number) = seen.get(&key) {
                    invalid.push(InvalidPaymentMethodFingerprintIdRecord {
                        line_number: record.line_number,
                        original_data: record.original_data,
                        invalid_reason: PaymentMethodFingerprintIdInvalidReason::DuplicateRecord {
                            first_line_number: *first_line_number,
                        },
                        failed_at_stage: PaymentMethodFingerprintIdInvalidStage::Validation,
                    });
                } else {
                    seen.insert(key, record.line_number.value());
                    valid.push(record);
                }
            }
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

    println!("Validation Results:");
    println!("  valid: {}", output.record_count);
    println!("  invalid: {}", invalid.len());
    println!();
    println!("Output saved:");
    println!("  -> {}", output_path.display());
    println!();
    println!("VALIDATE stage complete");
    println!("Next step:");
    println!("  migratus enrich {}", config_path.display());

    Ok(())
}

pub async fn handle_enrich(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("ENRICH Stage - Payment Method Fingerprint ID Migration");
    println!("======================================================\n");

    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;

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

    let validated_output: IntermediateOutput<PaymentMethodFingerprintIdMigrationRecord> =
        serde_json::from_str(&validated_json)?;

    let (records_for_migration, skipped_records) =
        filter_already_migrated_records(&config, validated_output.records)?;
    if !skipped_records.is_empty() {
        write_already_migrated_records_json_and_csv(&config, config_path, &skipped_records)?;
    }

    let output =
        IntermediateOutput::new(calculate_config_hash(config_path)?, records_for_migration);
    let output_path = config
        .output_config
        .output_dir
        .join("enriched_records.json");
    fs::write(&output_path, serde_json::to_string_pretty(&output)?)?;

    if let Some(already_migrated) = already_migrated_config(&config) {
        println!(
            "Filtered already migrated payment methods using {}",
            already_migrated.path.display()
        );
        println!("  skipped: {}", skipped_records.len());
        println!("  remaining: {}", output.record_count);
    } else {
        println!("No enrichment filter configured for payment method fingerprint ID migration");
    }
    println!("Output saved:");
    println!("  -> {}", output_path.display());
    println!("  -> {} records", output.record_count);
    println!();
    println!("ENRICH stage complete");
    println!("Next step:");
    println!("  migratus batch {}", config_path.display());

    Ok(())
}

pub async fn handle_batch(
    config_path: &Path,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("BATCH Stage - Payment Method Fingerprint ID Migration");
    println!("=====================================================\n");

    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;

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

    let enriched_output: IntermediateOutput<PaymentMethodFingerprintIdMigrationRecord> =
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

    println!("Batching Results:");
    println!("  batch files: {}", batches.len());
    println!("  batch size limit: {}", config.batch_config.batch_size);
    println!(
        "  file size limit: {} bytes",
        config.batch_config.max_file_size_bytes
    );
    println!("  -> {}", batches_dir.display());
    println!();
    println!("BATCH stage complete");
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
    println!("MIGRATE Stage - Payment Method Fingerprint ID Migration");
    println!("=======================================================\n");

    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;

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
        println!("All batches already migrated");
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

    println!("Migration Plan:");
    println!("  start batch: {}", start_batch);
    println!("  end batch: {}", end_batch);
    println!("  batches to upload: {}", jobs.len());
    println!(
        "  parallel uploads: {}",
        config.batch_config.parallel_uploads
    );
    println!();

    if jobs.is_empty() {
        println!("Selected batches already have responses");
        return Ok(());
    }

    let client = PaymentMethodFingerprintIdApiClient::new(
        config.api_config.endpoint.clone(),
        config.api_config.api_key.clone(),
        config.api_config.headers(),
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
            Ok(Ok(())) => saved += 1,
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

    println!("Migration Results:");
    println!("  response files saved: {}", saved);
    println!();
    println!("MIGRATE stage complete");
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
    println!("COMPLETE Stage - Payment Method Fingerprint ID Migration");
    println!("========================================================\n");

    let config = load_config(config_path)?;
    ensure_fingerprint_flow(&config)?;

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
    let enriched_count = read_record_count(
        &config
            .output_config
            .output_dir
            .join("enriched_records.json"),
    )?;
    let invalid_records = read_invalid_records_json(&config.output_config.output_dir)?;
    let skipped_records = read_already_migrated_records_json(&config.output_config.output_dir)?;
    let total_batches = list_batch_files(&config.output_config.output_dir.join("batches"))?.len();

    let mut per_status_counts: BTreeMap<PaymentMethodFingerprintIdStatus, usize> = BTreeMap::new();
    let mut api_total_rows = 0usize;
    let mut successful_count = 0usize;
    let mut failed_count = 0usize;
    let mut transport_error_count = 0usize;
    let mut jsonl_lines = Vec::new();
    let mut successful_records = Vec::new();
    let mut failed_records = Vec::new();

    for response_path in list_response_files(response_dir.path())? {
        let response_json = fs::read_to_string(&response_path)?;
        let saved: SavedPaymentMethodFingerprintIdBatchResponse =
            serde_json::from_str(&response_json)?;

        if let Some(error) = &saved.transport_error {
            *per_status_counts
                .entry(PaymentMethodFingerprintIdStatus::TransportError)
                .or_default() += saved.record_count;
            transport_error_count += saved.record_count;
            failed_count += saved.record_count;
            failed_records.push(FingerprintFailedOutputRecord {
                row_number: None,
                batch_number: saved.batch_number,
                batch_file: saved.batch_file.clone(),
                merchant_id: None,
                payment_method_id: None,
                old_fingerprint_id: None,
                new_fingerprint_id: None,
                migration_status: PaymentMethodFingerprintIdStatus::TransportError,
                error: Some(error.clone()),
            });
            jsonl_lines.push(serde_json::to_string(
                &PaymentMethodFingerprintIdJsonlResult {
                    batch_number: saved.batch_number,
                    batch_file: saved.batch_file.clone(),
                    row_number: None,
                    merchant_id: None,
                    payment_method_id: None,
                    old_fingerprint_id: None,
                    new_fingerprint_id: None,
                    migration_status: PaymentMethodFingerprintIdStatus::TransportError,
                    error: Some(error.clone()),
                    extra: BTreeMap::new(),
                },
            )?);
            continue;
        }

        let parsed = parse_fingerprint_response_body(&saved.body)?;
        api_total_rows += parsed.total_rows;
        successful_count += parsed.successful_count;
        failed_count += parsed.failed_count;

        for result in parsed.results {
            record_row_result(
                &saved,
                result,
                &mut per_status_counts,
                &mut successful_records,
                &mut failed_records,
                &mut jsonl_lines,
            )?;
        }
    }

    let summary = PaymentMethodFingerprintIdMigrationSummary {
        total_input_rows: merged_count,
        valid_rows: valid_count,
        enriched_rows: enriched_count,
        invalid_input_rows: invalid_records.len(),
        already_migrated_rows: skipped_records.len(),
        total_batches,
        api_total_rows,
        successful_count,
        failed_count,
        transport_error_count,
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
    write_successful_records_csv(
        &config
            .output_config
            .output_dir
            .join("successful_migrations.csv"),
        &successful_records,
    )?;
    write_failed_records_csv(
        &config
            .output_config
            .output_dir
            .join("failed_migrations.csv"),
        &failed_records,
    )?;

    println!("Final Summary:");
    println!("  total input rows: {}", summary.total_input_rows);
    println!("  valid rows: {}", summary.valid_rows);
    println!("  enriched rows: {}", summary.enriched_rows);
    println!("  invalid input rows: {}", summary.invalid_input_rows);
    println!("  already migrated rows: {}", summary.already_migrated_rows);
    println!("  total batches: {}", summary.total_batches);
    println!("  successful: {}", summary.successful_count);
    println!("  failed: {}", summary.failed_count);
    println!("  transport errors: {}", summary.transport_error_count);
    println!();
    println!("COMPLETE stage finished");

    Ok(())
}

fn record_row_result(
    saved: &SavedPaymentMethodFingerprintIdBatchResponse,
    result: PaymentMethodFingerprintIdApiRowResult,
    per_status_counts: &mut BTreeMap<PaymentMethodFingerprintIdStatus, usize>,
    successful_records: &mut Vec<FingerprintSuccessfulOutputRecord>,
    failed_records: &mut Vec<FingerprintFailedOutputRecord>,
    jsonl_lines: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let row_number = result.row_number;
    let merchant_id = result.merchant_id.clone();
    let payment_method_id = result.payment_method_id.clone();
    let old_fingerprint_id = result.old_fingerprint_id.clone();
    let new_fingerprint_id = result.new_fingerprint_id.clone();
    let migration_status = result.migration_status.clone();
    let failure_reason = result.failure_reason();
    let extra = result.extra.clone();

    *per_status_counts
        .entry(migration_status.clone())
        .or_default() += 1;

    if migration_status.is_success() {
        successful_records.push(FingerprintSuccessfulOutputRecord {
            row_number,
            batch_number: saved.batch_number,
            batch_file: saved.batch_file.clone(),
            merchant_id: merchant_id.clone(),
            payment_method_id: payment_method_id.clone(),
            old_fingerprint_id: old_fingerprint_id.clone(),
            new_fingerprint_id: new_fingerprint_id.clone(),
            migration_status: migration_status.clone(),
        });
    } else {
        failed_records.push(FingerprintFailedOutputRecord {
            row_number,
            batch_number: saved.batch_number,
            batch_file: saved.batch_file.clone(),
            merchant_id: merchant_id.clone(),
            payment_method_id: payment_method_id.clone(),
            old_fingerprint_id: old_fingerprint_id.clone(),
            new_fingerprint_id: new_fingerprint_id.clone(),
            migration_status: migration_status.clone(),
            error: failure_reason.clone(),
        });
    }

    jsonl_lines.push(serde_json::to_string(
        &PaymentMethodFingerprintIdJsonlResult {
            batch_number: saved.batch_number,
            batch_file: saved.batch_file.clone(),
            row_number,
            merchant_id,
            payment_method_id,
            old_fingerprint_id,
            new_fingerprint_id,
            migration_status,
            error: failure_reason,
            extra,
        },
    )?);

    Ok(())
}

fn load_config(config_path: &Path) -> Result<MigrationConfig, Box<dyn std::error::Error>> {
    let config_json = fs::read_to_string(config_path)?;
    Ok(serde_json::from_str(&config_json)?)
}

fn ensure_fingerprint_flow(config: &MigrationConfig) -> Result<(), Box<dyn std::error::Error>> {
    if is_payment_method_fingerprint_id_config(config) {
        Ok(())
    } else {
        Err("Config flow is not payment_method_fingerprint".into())
    }
}

fn already_migrated_config(config: &MigrationConfig) -> Option<AlreadyMigratedConfig> {
    if let Some(already_migrated) = config
        .enrichment
        .as_ref()
        .and_then(|enrichment| enrichment.already_migrated())
    {
        return Some(already_migrated);
    }

    config
        .enrichment
        .as_ref()
        .and_then(|enrichment| {
            enrichment
                .values
                .get(ALREADY_MIGRATED_PAYMENT_METHODS_PATH_KEY)
                .and_then(|value| value.as_str())
        })
        .filter(|path| !path.trim().is_empty())
        .map(|path| AlreadyMigratedConfig {
            path: PathBuf::from(path),
            match_fields: vec![MigrationField::PaymentMethodId],
        })
}

fn filter_already_migrated_records(
    config: &MigrationConfig,
    records: Vec<PaymentMethodFingerprintIdMigrationRecord>,
) -> Result<
    (
        Vec<PaymentMethodFingerprintIdMigrationRecord>,
        Vec<SkippedPaymentMethodFingerprintIdRecord>,
    ),
    Box<dyn std::error::Error>,
> {
    let Some(already_migrated_config) = already_migrated_config(config) else {
        return Ok((records, Vec::new()));
    };

    if already_migrated_config.match_fields.is_empty() {
        return Err("enrichment.already_migrated.match_fields must not be empty".into());
    }

    let match_fields: Vec<String> = already_migrated_config
        .match_fields
        .iter()
        .map(|field| field.to_header_name())
        .collect();
    let already_migrated =
        read_already_migrated_keys_for_fields(&already_migrated_config.path, &match_fields)?;
    if already_migrated.is_empty() {
        return Ok((records, Vec::new()));
    }

    let mut records_for_migration = Vec::new();
    let mut skipped_records = Vec::new();

    for record in records {
        let key = fingerprint_record_key(&record, &match_fields);
        if key
            .as_ref()
            .map(|key| already_migrated.contains(key))
            .unwrap_or(false)
        {
            skipped_records.push(SkippedPaymentMethodFingerprintIdRecord {
                line_number: record.line_number,
                merchant_id: record.merchant_id,
                payment_method_id: record.payment_method_id,
                original_data: record.original_data,
                skip_reason: PaymentMethodFingerprintIdSkipReason::AlreadyMigrated,
            });
        } else {
            records_for_migration.push(record);
        }
    }

    Ok((records_for_migration, skipped_records))
}

fn read_already_migrated_keys_for_fields(
    path: &Path,
    match_fields: &[String],
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)?;
    let mut keys = HashSet::new();
    let mut field_indexes = None;

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

fn fingerprint_record_key(
    record: &PaymentMethodFingerprintIdMigrationRecord,
    match_fields: &[String],
) -> Option<String> {
    let values: Option<Vec<String>> = match_fields
        .iter()
        .map(|field| {
            match field.as_str() {
                "merchant_id" => Some(record.merchant_id.inner().to_string()),
                "payment_method_id" => Some(record.payment_method_id.inner().to_string()),
                _ => record.original_data.get(field).cloned(),
            }
            .map(|value| value.trim().to_string())
        })
        .collect();

    values.and_then(|values| {
        if values.iter().all(|value| !value.is_empty()) {
            Some(values.join("\u{1f}"))
        } else {
            None
        }
    })
}

fn write_invalid_records_json_and_csv(
    config: &MigrationConfig,
    config_path: &Path,
    records: &[InvalidPaymentMethodFingerprintIdRecord],
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

fn write_already_migrated_records_json_and_csv(
    config: &MigrationConfig,
    config_path: &Path,
    records: &[SkippedPaymentMethodFingerprintIdRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = IntermediateOutput::new(calculate_config_hash(config_path)?, records.to_vec());
    fs::write(
        config
            .output_config
            .output_dir
            .join(ALREADY_MIGRATED_RECORDS_JSON),
        serde_json::to_string_pretty(&output)?,
    )?;
    write_already_migrated_records_csv(
        &config
            .output_config
            .output_dir
            .join("already_migrated_records.csv"),
        records,
    )?;
    Ok(())
}

fn write_invalid_records_csv(
    path: &Path,
    records: &[InvalidPaymentMethodFingerprintIdRecord],
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

fn write_already_migrated_records_csv(
    path: &Path,
    records: &[SkippedPaymentMethodFingerprintIdRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "line_number",
        "merchant_id",
        "payment_method_id",
        "skip_reason",
        "data",
    ])?;
    for record in records {
        writer.write_record([
            record.line_number.value().to_string(),
            record.merchant_id.inner().to_string(),
            record.payment_method_id.inner().to_string(),
            record.skip_reason.to_string(),
            serde_json::to_string(&record.original_data)?,
        ])?;
    }
    writer.flush()?;
    Ok(())
}

fn write_successful_records_csv(
    path: &Path,
    records: &[FingerprintSuccessfulOutputRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "batch_number",
        "batch_file",
        "row_number",
        "merchant_id",
        "payment_method_id",
        "old_fingerprint_id",
        "new_fingerprint_id",
        "migration_status",
    ])?;

    for record in records {
        writer.write_record([
            record.batch_number.to_string(),
            record.batch_file.clone(),
            record
                .row_number
                .map(|row_number| row_number.to_string())
                .unwrap_or_default(),
            record
                .merchant_id
                .as_ref()
                .map(|merchant_id| merchant_id.inner().to_string())
                .unwrap_or_default(),
            record
                .payment_method_id
                .as_ref()
                .map(|payment_method_id| payment_method_id.inner().to_string())
                .unwrap_or_default(),
            record.old_fingerprint_id.clone().unwrap_or_default(),
            record.new_fingerprint_id.clone().unwrap_or_default(),
            record.migration_status.to_string(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

fn write_failed_records_csv(
    path: &Path,
    records: &[FingerprintFailedOutputRecord],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "batch_number",
        "batch_file",
        "row_number",
        "merchant_id",
        "payment_method_id",
        "old_fingerprint_id",
        "new_fingerprint_id",
        "migration_status",
        "error",
    ])?;

    for record in records {
        writer.write_record([
            record.batch_number.to_string(),
            record.batch_file.clone(),
            record
                .row_number
                .map(|row_number| row_number.to_string())
                .unwrap_or_default(),
            record
                .merchant_id
                .as_ref()
                .map(|merchant_id| merchant_id.inner().to_string())
                .unwrap_or_default(),
            record
                .payment_method_id
                .as_ref()
                .map(|payment_method_id| payment_method_id.inner().to_string())
                .unwrap_or_default(),
            record.old_fingerprint_id.clone().unwrap_or_default(),
            record.new_fingerprint_id.clone().unwrap_or_default(),
            record.migration_status.to_string(),
            record.error.clone().unwrap_or_default(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

fn read_invalid_records_json(
    output_dir: &Path,
) -> Result<Vec<InvalidPaymentMethodFingerprintIdRecord>, Box<dyn std::error::Error>> {
    let path = output_dir.join(INVALID_RECORDS_JSON);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(path)?;
    let output: IntermediateOutput<InvalidPaymentMethodFingerprintIdRecord> =
        serde_json::from_str(&json)?;
    Ok(output.records)
}

fn read_already_migrated_records_json(
    output_dir: &Path,
) -> Result<Vec<SkippedPaymentMethodFingerprintIdRecord>, Box<dyn std::error::Error>> {
    let path = output_dir.join(ALREADY_MIGRATED_RECORDS_JSON);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(path)?;
    let output: IntermediateOutput<SkippedPaymentMethodFingerprintIdRecord> =
        serde_json::from_str(&json)?;
    Ok(output.records)
}

fn list_batch_files(
    dir: &Path,
) -> Result<Vec<PaymentMethodFingerprintIdBatchFile>, Box<dyn std::error::Error>> {
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
        batches.push(PaymentMethodFingerprintIdBatchFile {
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
    client: PaymentMethodFingerprintIdApiClient,
    response_dir: PathBuf,
    job: PaymentMethodFingerprintIdBatchFile,
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

    let saved = SavedPaymentMethodFingerprintIdBatchResponse {
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

fn parse_fingerprint_response_body(
    body: &serde_json::Value,
) -> Result<PaymentMethodFingerprintIdApiResponse, Box<dyn std::error::Error>> {
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
    fn parses_sample_response_body() {
        let body = serde_json::json!({
            "total_rows": 1,
            "successful_count": 1,
            "failed_count": 0,
            "results": [{
                "row_number": 2,
                "merchant_id": "merchant_1",
                "payment_method_id": "pm_1",
                "old_fingerprint_id": "fp_old",
                "new_fingerprint_id": "fp_new",
                "migration_status": "Success"
            }]
        });

        let parsed = parse_fingerprint_response_body(&body).unwrap();
        assert_eq!(parsed.total_rows, 1);
        assert_eq!(parsed.successful_count, 1);
        assert_eq!(parsed.failed_count, 0);
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(
            parsed.results[0].migration_status,
            PaymentMethodFingerprintIdStatus::Success
        );
    }

    #[test]
    fn parses_failed_response_row_and_preserves_unknown_fields() {
        let body = serde_json::json!({
            "total_rows": 1,
            "successful_count": 0,
            "failed_count": 1,
            "results": [{
                "row_number": 2,
                "merchant_id": "merchant_1",
                "payment_method_id": "pm_1",
                "old_fingerprint_id": "fp_old",
                "new_fingerprint_id": null,
                "migration_status": "Failed",
                "message": "not found",
                "debug_code": "missing_pm"
            }]
        });

        let parsed = parse_fingerprint_response_body(&body).unwrap();
        let result = &parsed.results[0];
        assert_eq!(
            result.migration_status,
            PaymentMethodFingerprintIdStatus::Unknown("Failed".to_string())
        );
        assert_eq!(result.failure_reason(), Some("not found".to_string()));
        assert_eq!(
            result.extra.get("debug_code"),
            Some(&serde_json::Value::String("missing_pm".to_string()))
        );
    }

    #[test]
    fn reads_already_migrated_ids_from_csv_or_line_list() {
        let dir = std::env::temp_dir().join(format!(
            "migratus-fingerprint-filter-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();

        let csv_path = dir.join("already.csv");
        fs::write(
            &csv_path,
            "merchant_id,payment_method_id\nmerchant_1,pm_csv_1\nmerchant_2,pm_csv_2\n",
        )
        .unwrap();
        let ids =
            read_already_migrated_keys_for_fields(&csv_path, &["payment_method_id".to_string()])
                .unwrap();
        assert!(ids.contains("pm_csv_1"));
        assert!(ids.contains("pm_csv_2"));
        assert!(!ids.contains("merchant_1"));

        let list_path = dir.join("already.txt");
        fs::write(&list_path, "pm_line_1\npm_line_2\n\n").unwrap();
        let ids =
            read_already_migrated_keys_for_fields(&list_path, &["payment_method_id".to_string()])
                .unwrap();
        assert!(ids.contains("pm_line_1"));
        assert!(ids.contains("pm_line_2"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn no_already_migrated_config_keeps_all_records() {
        let config: MigrationConfig = serde_json::from_value(serde_json::json!({
            "flow": { "type": "payment_method_fingerprint" },
            "data_source": { "type": "merged", "path": "input.csv" },
            "api_config": {
                "endpoint": "http://localhost/payment_methods/fingerprint/migrate-batch",
                "api_key": "secret",
                "merchant_connector_ids": null
            },
            "batch_config": { "batch_size": 500 },
            "output_config": {
                "output_dir": "output",
                "batch_response_dir": "output/batch_responses"
            }
        }))
        .unwrap();
        let record = PaymentMethodFingerprintIdMigrationRecord {
            line_number: crate::domain::types::LineNumber::new(2),
            merchant_id: crate::domain::types::MerchantId::new("merchant_1".to_string()),
            payment_method_id: crate::domain::types::PaymentMethodId::new("pm_1".to_string()),
            original_data: HashMap::new(),
        };

        let (kept, skipped) = filter_already_migrated_records(&config, vec![record]).unwrap();
        assert_eq!(kept.len(), 1);
        assert!(skipped.is_empty());
    }

    #[tokio::test]
    async fn runs_local_flow_through_complete_with_saved_response() {
        let dir = std::env::temp_dir().join(format!(
            "migratus-fingerprint-flow-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("input.csv");
        let output_dir = dir.join("output");
        let response_dir = output_dir.join("batch_responses");
        let config_path = dir.join("config.json");
        let already_migrated_path = dir.join("already_migrated.csv");

        fs::write(
            &input_path,
            concat!(
                "merchant_id,payment_method_id\n",
                "merchant_1,pm_1\n",
                "merchant_1,pm_2\n",
                "merchant_1,pm_1\n"
            ),
        )
        .unwrap();
        fs::write(&already_migrated_path, "payment_method_id\npm_2\n").unwrap();

        let config = serde_json::json!({
            "flow": { "type": "payment_method_fingerprint" },
            "data_source": {
                "type": "merged",
                "path": input_path
            },
            "api_config": {
                "endpoint": "http://localhost/payment_methods/fingerprint/migrate-batch",
                "api_key": "secret",
                "merchant_connector_ids": null,
                "timeout_secs": 30
            },
            "batch_config": {
                "batch_size": 500,
                "parallel_uploads": 1,
                "max_file_size_bytes": 1048576,
                "retry_count": 0,
                "retry_backoff_ms": 1
            },
            "output_config": {
                "output_dir": output_dir,
                "batch_response_dir": response_dir
            },
            "enrichment": {
                "already_migrated": {
                    "path": already_migrated_path,
                    "match_fields": ["payment_method_id"]
                }
            }
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

        handle_load(&config_path).await.unwrap();
        handle_validate(&config_path, false).await.unwrap();
        handle_enrich(&config_path, false).await.unwrap();
        handle_batch(&config_path, false).await.unwrap();

        fs::create_dir_all(&response_dir).unwrap();
        let saved = SavedPaymentMethodFingerprintIdBatchResponse {
            batch_number: 1,
            batch_file: "batch_0001.csv".to_string(),
            record_count: 1,
            byte_size: fs::metadata(output_dir.join("batches/batch_0001.csv"))
                .unwrap()
                .len() as usize,
            endpoint: "http://localhost/payment_methods/fingerprint/migrate-batch".to_string(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            completed_at: "2026-01-01T00:00:01Z".to_string(),
            attempts: 1,
            http_status: Some(200),
            headers: HashMap::new(),
            body: serde_json::json!({
                "total_rows": 1,
                "successful_count": 1,
                "failed_count": 0,
                "results": [
                    {
                        "row_number": 2,
                        "merchant_id": "merchant_1",
                        "payment_method_id": "pm_1",
                        "old_fingerprint_id": "fp_1",
                        "new_fingerprint_id": "fp_1",
                        "migration_status": "Success"
                    }
                ]
            }),
            transport_error: None,
        };
        fs::write(
            response_dir.join("batch_0001.json"),
            serde_json::to_string_pretty(&saved).unwrap(),
        )
        .unwrap();

        handle_complete(&config_path).await.unwrap();

        let summary: PaymentMethodFingerprintIdMigrationSummary =
            serde_json::from_str(&fs::read_to_string(output_dir.join("summary.json")).unwrap())
                .unwrap();
        assert_eq!(summary.total_input_rows, 3);
        assert_eq!(summary.valid_rows, 2);
        assert_eq!(summary.enriched_rows, 1);
        assert_eq!(summary.invalid_input_rows, 1);
        assert_eq!(summary.already_migrated_rows, 1);
        assert_eq!(summary.successful_count, 1);
        assert_eq!(summary.failed_count, 0);
        assert!(output_dir.join("already_migrated_records.csv").exists());
        assert!(output_dir.join("successful_migrations.csv").exists());
        assert!(output_dir.join("failed_migrations.csv").exists());
        assert!(output_dir.join("migration_results.jsonl").exists());

        fs::remove_dir_all(dir).unwrap();
    }
}
