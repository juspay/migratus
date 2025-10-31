use migratus::domain::config::*;
use migratus::machine::builder::MigrationBuilder;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example configuration for a basic migration
    let config = MigrationConfig {
        flow: MigrationFlow::RawCard,
        data_source: DataSource::Merged {
            path: PathBuf::from("input/merged_data.csv"),
        },
        api_config: ApiConfig {
            endpoint: "https://api.example.com/migrate".to_string(),
            api_key: "your_api_key_here".to_string(),
            merchant_id: "merchant_123".to_string(),
            merchant_connector_id: Some("mca_456".to_string()),
            timeout_secs: 30,
        },
        batch_config: BatchConfig {
            batch_size: 100,
            resume_from_batch: None,
            resume_from_state: None,
        },
        output_config: OutputConfig {
            output_dir: PathBuf::from("output"),
            batch_response_dir: PathBuf::from("output/batch_responses"),
            output_fields: None,
        },
    };

    println!("Starting migration pipeline...");

    // Initialize the migration builder
    let builder = MigrationBuilder::new(config);

    // Initialize and determine if merge is needed
    let decision = builder.initialize()?;

    println!("Initialization complete");

    // Process based on decision
    let validated = match decision {
        migratus::machine::builder::BranchDecision::RequiresMerge(b) => {
            println!("Merging customer and payment data...");
            b.merge().await?
        }
        migratus::machine::builder::BranchDecision::SkipMerge(b) => {
            println!("Loading pre-merged data...");
            b.load_merged_data().await?
        }
    };

    println!("Data loaded and merged");

    // Validate records
    let enriched = validated.validate().await?;
    println!("Validation complete");

    // Enrich with additional columns
    let mut columns = EnrichmentColumns::new();
    columns.add("merchant_id".to_string(), "merchant_123".to_string());
    columns.add("merchant_connector_id".to_string(), "mca_456".to_string());

    let batched = enriched.enrich(columns).await?;
    println!("Enrichment complete");

    // Create batches
    let migrated = batched.batch().await?;
    println!("Batching complete");

    // Execute migration
    let completed = migrated.migrate().await?;
    println!("Migration complete");

    // Finalize and write outputs
    let output = completed.complete().await?;

    println!("\n=== Migration Summary ===");
    println!(
        "Total input records: {}",
        output.summary.total_input_records
    );
    println!(
        "Successful migrations: {}",
        output.summary.successful_migrations
    );
    println!("Failed migrations: {}", output.summary.failed_migrations);
    println!(
        "Invalid records (pre-migration): {}",
        output.summary.invalid_pre_migration
    );
    println!("\nOutput files written to: output/");
    println!("  - successful_migrations.csv");
    println!("  - failed_migrations.csv");
    println!("  - invalid_records.csv");

    Ok(())
}
