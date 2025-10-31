use crate::domain::config::MigrationConfig;
use crate::domain::records::Batch;
use crate::domain::types::BatchNumber;
use crate::operations::api_client::ApiClient;
use crate::operations::csv_reader::CsvReader;
use crate::utils::hash::verify_config_hash;
use crate::utils::state::{get_completed_batches, get_next_batch_to_migrate, get_total_batches};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;

pub async fn handle_migrate(
    config_path: &Path,
    from_batch: Option<usize>,
    count: usize,
    all: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 MIGRATE Stage");
    println!("================\n");
    
    // Load configuration
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;
    
    // Verify enriched records exist and hash matches
    let enriched_path = config.output_config.output_dir.join("enriched_records.json");
    if enriched_path.exists() {
        let enriched_json = fs::read_to_string(&enriched_path)?;
        if !force && !verify_config_hash(&enriched_json, config_path)? {
            return Err(
                "Config file has changed since ENRICH stage. Use --force to override or re-run from LOAD"
                    .into(),
            );
        }
    }
    
    // Check batches exist
    let batches_dir = config.output_config.output_dir.join("batches");
    if !batches_dir.exists() {
        return Err(format!(
            "Batches not found. Run 'migratus batch {}' first",
            config_path.display()
        )
        .into());
    }
    
    // Get batch info
    let total_batches = get_total_batches(&config.output_config.output_dir);
    let completed_batches = get_completed_batches(&config.output_config.output_dir);
    
    println!("📊 Migration Status:");
    println!("  Total batches: {}", total_batches);
    println!("  Completed: {}", completed_batches.len());
    println!("  Remaining: {}", total_batches - completed_batches.len());
    println!();
    
    // Determine start batch
    let start_batch = from_batch.unwrap_or_else(|| {
        get_next_batch_to_migrate(&config.output_config.output_dir)
    });
    
    // Determine end batch
    let end_batch = if all {
        total_batches
    } else {
        std::cmp::min(start_batch + count - 1, total_batches)
    };
    
    if start_batch > total_batches {
        println!("✅ All batches already migrated!");
        return Ok(());
    }
    
    let batches_to_migrate = end_batch - start_batch + 1;
    
    println!("🎯 Migration Plan:");
    println!("  Start batch: {}", start_batch);
    println!("  End batch: {}", end_batch);
    println!("  Batches to migrate: {}", batches_to_migrate);
    println!();
    
    // Create API client
    let api_client = ApiClient::new(
        config.api_config.endpoint.clone(),
        config.api_config.api_key.clone(),
        config.api_config.merchant_id.clone(),
        config.api_config.merchant_connector_ids.clone(),
        config.api_config.timeout(),
    )?;
    
    // Create batch responses directory
    let batch_response_dir = config.output_config.batch_response_dir();
    fs::create_dir_all(batch_response_dir.path())?;
    
    // Progress bar
    let pb = ProgressBar::new(batches_to_migrate as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );
    
    println!("🔄 Migrating batches {}..{}", start_batch, end_batch);
    println!();
    
    let mut successful = 0;
    
    for batch_num in start_batch..=end_batch {
        // Check if already migrated
        let response_file = batch_response_dir
            .path()
            .join(format!("batch_{:04}.json", batch_num));
        
        if response_file.exists() && !force {
            pb.inc(1);
            successful += 1;
            continue;
        }
        
        // Read batch CSV
        let batch_file = batches_dir.join(format!("batch_{:04}.csv", batch_num));
        let mut csv_reader = CsvReader::new();
        let records = csv_reader.read_file(&batch_file)?;
        
        // Convert to Batch
        let enriched_records: Vec<_> = records
            .into_iter()
            .map(|r| crate::domain::records::EnrichedRecord::new(r.line_number, r.data))
            .collect();
        
        let batch = Batch::new(BatchNumber::new(batch_num), enriched_records);
        
        // Migrate batch
        match api_client.migrate_batch_with_headers(&batch).await {
            Ok((response, headers)) => {
                // Check if response is an error (non-2xx status)
                if let crate::operations::api_client::BatchMigrationResponse::Error { status, ref message } = response {
                    // Save error response
                    let response_data = serde_json::json!({
                        "batch_number": batch_num,
                        "headers": headers,
                        "body": response
                    });
                    let response_json = serde_json::to_string_pretty(&response_data)?;
                    fs::write(&response_file, response_json)?;
                    
                    // Halt migration
                    pb.finish_with_message("Migration halted due to error");
                    println!();
                    eprintln!("❌ Batch {} failed with status {}", batch_num, status);
                    eprintln!("   Error: {}", message);
                    println!();
                    println!("📊 Migration Results (before halt):");
                    println!("  ✓ Successful: {}", successful);
                    println!("  ✗ Failed: 1 (batch {})", batch_num);
                    println!();
                    println!("⚠️  Migration halted. Fix the issue and retry with:");
                    println!("  migratus migrate {} --from-batch {}", 
                        config_path.display(), batch_num);
                    return Err(format!("Batch {} failed with status {}: {}", batch_num, status, message).into());
                }
                
                // Save successful response
                let response_data = serde_json::json!({
                    "batch_number": batch_num,
                    "headers": headers,
                    "body": response
                });
                
                let response_json = serde_json::to_string_pretty(&response_data)?;
                fs::write(&response_file, response_json)?;
                
                successful += 1;
            }
            Err(e) => {
                pb.finish_with_message("Migration halted due to error");
                println!();
                eprintln!("❌ Batch {} failed: {}", batch_num, e);
                println!();
                println!("📊 Migration Results (before halt):");
                println!("  ✓ Successful: {}", successful);
                println!("  ✗ Failed: 1 (batch {})", batch_num);
                println!();
                println!("⚠️  Migration halted. Fix the issue and retry with:");
                println!("  migratus migrate {} --from-batch {}", 
                    config_path.display(), batch_num);
                return Err(Box::new(e));
            }
        }
        
        pb.inc(1);
    }
    
    pb.finish_with_message("Migration complete");
    println!();
    
    println!("📊 Migration Results:");
    println!("  ✓ Successful: {}", successful);
    println!();
    
    if end_batch < total_batches {
        println!("▶️  Continue migration with:");
        println!("  migratus migrate {} --from-batch {} --count {}", 
            config_path.display(), end_batch + 1, count);
    } else {
        println!("✅ All batches migrated!");
        println!();
        println!("Next step:");
        println!("  migratus complete {}", config_path.display());
    }
    
    Ok(())
}
