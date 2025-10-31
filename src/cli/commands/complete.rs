use crate::domain::config::{MigrationConfig, MigrationFlow};
use crate::domain::records::{MigrationResults, SuccessfulMigration, FailedMigration, MigrationFailureReason, MigrationMetadata};
use crate::domain::types::{LineNumber, BatchNumber, PaymentMethodId, CustomerId};
use crate::domain::update::{UpdateResults, SuccessfulUpdate, FailedUpdate, UpdateFailureReason, UpdateMetadata};
use crate::operations::api_client::ApiMigrationRecord;
use crate::operations::api::ApiUpdateResponse;
use crate::operations::csv_writer::CsvWriter;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub async fn handle_complete(
    config_path: &Path,
    _force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📝 COMPLETE Stage");
    println!("=================\n");
    
    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;
    
    // Route based on flow type
    match config.flow {
        MigrationFlow::Update => handle_complete_update(config_path, config).await,
        _ => handle_complete_migrate(config_path, config).await,
    }
}

async fn handle_complete_migrate(
    config_path: &Path,
    config: MigrationConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    
    // Check batch responses exist
    let batch_response_dir = config.output_config.batch_response_dir();
    if !batch_response_dir.path().exists() {
        return Err(format!(
            "Batch responses not found. Run 'migratus migrate {}' first",
            config_path.display()
        )
        .into());
    }
    
    // Check batch files exist
    let batch_dir = config.output_config.output_dir.join("batches");
    if !batch_dir.exists() {
        return Err(format!(
            "Batch files not found. Run 'migratus load {}' first",
            config_path.display()
        )
        .into());
    }
    
    println!("📂 Processing batch files and responses...");
    
    let mut migration_results = MigrationResults::new();
    let mut batch_count = 0;
    let mut unmatched_count = 0;
    
    // Read all batch response files
    let entries = fs::read_dir(batch_response_dir.path())?;
    let mut response_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    
    // Sort by filename to process in order
    response_files.sort_by_key(|e| e.file_name());
    
    for entry in response_files {
        let response_path = entry.path();
        let response_json = fs::read_to_string(&response_path)?;
        let response_data: serde_json::Value = serde_json::from_str(&response_json)?;
        
        // Extract batch number
        let batch_num = response_data["batch_number"].as_u64().unwrap_or(0) as usize;
        
        // Find corresponding batch CSV file
        let batch_filename = format!("batch_{:04}.csv", batch_num);
        let batch_csv_path = batch_dir.join(&batch_filename);
        
        if !batch_csv_path.exists() {
            eprintln!("  ⚠ Warning: Batch CSV file not found: {}", batch_filename);
            continue;
        }
        
        // Read batch CSV and create unique key -> original_data map
        // Use customer_id + card_number_masked as unique key (or payment_instrument_id if available)
        let mut csv_reader = csv::Reader::from_path(&batch_csv_path)?;
        let headers = csv_reader.headers()?.clone();
        
        let mut record_data_map: HashMap<String, HashMap<String, String>> = HashMap::new();
        
        for result in csv_reader.records() {
            let record = result?;
            let mut row_data = HashMap::new();
            
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    row_data.insert(header.to_string(), field.to_string());
                }
            }
            
            // Create unique key: customer_id + card_number_masked (to match API response format)
            let unique_key = if let (Some(cust_id), Some(card_masked)) = (row_data.get("customer_id"), row_data.get("card_number_masked")) {
                format!("{}|{}", cust_id, card_masked)
            } else {
                continue; // Skip records without identifiable keys
            };
            
            record_data_map.insert(unique_key, row_data);
        }
        
        // Process API response body
        let body = &response_data["body"];
        
        if let Some(records) = body.as_array() {
            for record_value in records {
                let record: ApiMigrationRecord = serde_json::from_value(record_value.clone())?;
                
                // Get line number for tracking
                let line_num = record.line_number.unwrap_or(1) as usize;
                
                // Build unique key from API response (customer_id + card_number_masked)
                let unique_key = if let (Some(cust_id), Some(card_masked)) = (&record.customer_id, &record.card_number_masked) {
                    format!("{}|{}", cust_id, card_masked)
                } else {
                    // No unique key available, skip this record
                    unmatched_count += 1;
                    continue;
                };
                
                // Try to find original data by unique key
                let original_data = record_data_map.get(&unique_key).cloned().unwrap_or_else(|| {
                    unmatched_count += 1;
                    HashMap::new()
                });
                
                if record.migration_status == "Success" {
                    if let (Some(pm_id), Some(cust_id)) = (&record.payment_method_id, &record.customer_id) {
                        migration_results.successful.push(SuccessfulMigration {
                            line_number: LineNumber::new(line_num),
                            batch_number: BatchNumber::new(batch_num),
                            original_data,
                            payment_method_id: PaymentMethodId::new(pm_id.clone()),
                            customer_id: CustomerId::new(cust_id.clone()),
                            migration_status: record.migration_status.clone(),
                            metadata: MigrationMetadata {
                                card_migrated: record.card_migrated,
                                network_token_migrated: record.network_token_migrated.unwrap_or(false),
                                connector_mandate_details_migrated: record.connector_mandate_details_migrated.unwrap_or(false),
                                network_transaction_id_migrated: record.network_transaction_id_migrated.unwrap_or(false),
                            },
                            payment_method: record.payment_method.clone(),
                            payment_method_type: record.payment_method_type.clone(),
                            card_number_masked: record.card_number_masked.clone(),
                        });
                    }
                } else {
                    migration_results.failed.push(FailedMigration {
                        line_number: LineNumber::new(line_num),
                        batch_number: BatchNumber::new(batch_num),
                        original_data,
                        failure_reason: MigrationFailureReason::RecordLevelFailure(
                            record.migration_error.unwrap_or_default(),
                        ),
                    });
                }
            }
        }
        
        batch_count += 1;
        println!("  ✓ Processed batch {}: {} records", batch_num, record_data_map.len());
    }
    
    if unmatched_count > 0 {
        println!("  ⚠ Warning: {} records could not be matched by unique key", unmatched_count);
    }
    
    println!();
    println!("  ✓ Processed {} batch files", batch_count);
    println!("  ✓ Successful migrations: {}", migration_results.successful.len());
    println!("  ✗ Failed migrations: {}", migration_results.failed.len());
    println!();
    
    // Generate output files
    println!("💾 Generating output files...");
    
    let writer = CsvWriter::new();
    
    // Write successful migrations
    let success_path = config.output_config.output_dir.join("successful_migrations.csv");
    if let Some(output_fields) = &config.output_config.output_fields {
        writer.write_successful_migrations_custom(&success_path, &migration_results.successful, output_fields)?;
    } else {
        writer.write_successful_migrations(&success_path, &migration_results.successful)?;
    }
    println!("  ✓ {}", success_path.display());
    
    // Write failed migrations
    let failed_path = config.output_config.output_dir.join("failed_migrations.csv");
    writer.write_failed_migrations(&failed_path, &migration_results.failed)?;
    println!("  ✓ {}", failed_path.display());
    
    // Write summary
    let summary = serde_json::json!({
        "total_batches": batch_count,
        "successful_migrations": migration_results.successful.len(),
        "failed_migrations": migration_results.failed.len(),
        "total_migrations": migration_results.total(),
        "unmatched_records": unmatched_count,
    });
    
    let summary_path = config.output_config.output_dir.join("summary.json");
    let summary_json = serde_json::to_string_pretty(&summary)?;
    fs::write(&summary_path, summary_json)?;
    println!("  ✓ {}", summary_path.display());
    println!();
    
    println!("📊 Final Summary:");
    println!("  Total batches processed: {}", batch_count);
    println!("  ✓ Successful: {}", migration_results.successful.len());
    println!("  ✗ Failed: {}", migration_results.failed.len());
    println!("  Total: {}", migration_results.total());
    if unmatched_count > 0 {
        println!("  ⚠ Unmatched: {}", unmatched_count);
    }
    println!();
    
    println!("✅ COMPLETE stage finished!");
    println!();
    println!("🎉 Migration pipeline complete!");
    println!("   Check the output directory for final results.");
    
    Ok(())
}

async fn handle_complete_update(
    config_path: &Path,
    config: MigrationConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    
    // Check batch responses exist
    let batch_response_dir = config.output_config.batch_response_dir();
    if !batch_response_dir.path().exists() {
        return Err(format!(
            "Batch responses not found. Run 'updatus update {}' first",
            config_path.display()
        )
        .into());
    }
    
    // Check batch files exist
    let batch_dir = config.output_config.output_dir.join("batches");
    if !batch_dir.exists() {
        return Err(format!(
            "Batch files not found. Run 'updatus load {}' first",
            config_path.display()
        )
        .into());
    }
    
    println!("📂 Processing batch files and responses...");
    
    let mut update_results = UpdateResults::new();
    let mut batch_count = 0;
    let mut unmatched_count = 0;
    
    // Read all batch response files
    let entries = fs::read_dir(batch_response_dir.path())?;
    let mut response_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    
    // Sort by filename to process in order
    response_files.sort_by_key(|e| e.file_name());
    
    for entry in response_files {
        let response_path = entry.path();
        let response_json = fs::read_to_string(&response_path)?;
        let response_data: serde_json::Value = serde_json::from_str(&response_json)?;
        
        // Extract batch number
        let batch_num = response_data["batch_number"].as_u64().unwrap_or(0) as usize;
        
        // Find corresponding batch CSV file
        let batch_filename = format!("batch_{:04}.csv", batch_num);
        let batch_csv_path = batch_dir.join(&batch_filename);
        
        if !batch_csv_path.exists() {
            eprintln!("  ⚠ Warning: Batch CSV file not found: {}", batch_filename);
            continue;
        }
        
        // Read batch CSV and create payment_method_id -> original_data map
        let mut csv_reader = csv::Reader::from_path(&batch_csv_path)?;
        let headers = csv_reader.headers()?.clone();
        
        let mut pm_data_map: HashMap<String, HashMap<String, String>> = HashMap::new();
        
        for result in csv_reader.records() {
            let record = result?;
            let mut row_data = HashMap::new();
            
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    row_data.insert(header.to_string(), field.to_string());
                }
            }
            
            // Extract payment_method_id from the row
            if let Some(pm_id) = row_data.get("payment_method_id") {
                pm_data_map.insert(pm_id.clone(), row_data);
            }
        }
        
        // Process API response body
        let body = &response_data["body"];
        
        if let Some(records) = body.as_array() {
            for record_value in records {
                let record: ApiUpdateResponse = serde_json::from_value(record_value.clone())?;
                
                // Get line number for tracking
                let line_num = record.line_number.unwrap_or(1) as usize;
                
                // Match by payment_method_id
                let original_data = if let Some(pm_id) = &record.payment_method_id {
                    pm_data_map.get(pm_id).cloned().unwrap_or_else(|| {
                        unmatched_count += 1;
                        HashMap::new()
                    })
                } else {
                    unmatched_count += 1;
                    HashMap::new()
                };
                
                if record.update_status == "Success" {
                    if let Some(pm_id) = &record.payment_method_id {
                        update_results.successful.push(SuccessfulUpdate {
                            line_number: LineNumber::new(line_num),
                            batch_number: BatchNumber::new(batch_num),
                            original_data,
                            payment_method_id: PaymentMethodId::new(pm_id.clone()),
                            update_status: record.update_status.clone(),
                            metadata: UpdateMetadata {
                                updated_payment_method_data: record.updated_payment_method_data,
                                connector_customer_updated: record.connector_customer.is_some(),
                                connector_mandate_details_updated: record.connector_mandate_details.is_some(),
                            },
                        });
                    }
                } else {
                    update_results.failed.push(FailedUpdate {
                        line_number: LineNumber::new(line_num),
                        batch_number: BatchNumber::new(batch_num),
                        original_data,
                        failure_reason: UpdateFailureReason::RecordLevelFailure(
                            record.update_error.unwrap_or_default(),
                        ),
                    });
                }
            }
        }
        
        batch_count += 1;
        println!("  ✓ Processed batch {}: {} records", batch_num, pm_data_map.len());
    }
    
    if unmatched_count > 0 {
        println!("  ⚠ Warning: {} records could not be matched by payment_method_id", unmatched_count);
    }
    
    println!();
    println!("  ✓ Processed {} batch files", batch_count);
    println!("  ✓ Successful updates: {}", update_results.successful.len());
    println!("  ✗ Failed updates: {}", update_results.failed.len());
    println!();
    
    // Generate output files
    println!("💾 Generating output files...");
    
    let writer = CsvWriter::new();
    
    // Write successful updates
    let success_path = config.output_config.output_dir.join("successful_updates.csv");
    writer.write_successful_updates(&success_path, &update_results.successful)?;
    println!("  ✓ {}", success_path.display());
    
    // Write failed updates
    let failed_path = config.output_config.output_dir.join("failed_updates.csv");
    writer.write_failed_updates(&failed_path, &update_results.failed)?;
    println!("  ✓ {}", failed_path.display());
    
    // Write summary
    let summary = serde_json::json!({
        "total_batches": batch_count,
        "successful_updates": update_results.successful.len(),
        "failed_updates": update_results.failed.len(),
        "total_updates": update_results.total(),
        "unmatched_records": unmatched_count,
    });
    
    let summary_path = config.output_config.output_dir.join("summary.json");
    let summary_json = serde_json::to_string_pretty(&summary)?;
    fs::write(&summary_path, summary_json)?;
    println!("  ✓ {}", summary_path.display());
    println!();
    
    println!("📊 Final Summary:");
    println!("  Total batches processed: {}", batch_count);
    println!("  ✓ Successful: {}", update_results.successful.len());
    println!("  ✗ Failed: {}", update_results.failed.len());
    println!("  Total: {}", update_results.total());
    if unmatched_count > 0 {
        println!("  ⚠ Unmatched: {}", unmatched_count);
    }
    println!();
    
    println!("✅ COMPLETE stage finished!");
    println!();
    println!("🎉 Update pipeline complete!");
    println!("   Check the output directory for final results.");
    
    Ok(())
}
