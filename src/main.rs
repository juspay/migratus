use clap::Parser;
use migratus::cli::{Cli, Command};
use migratus::domain::config::MigrationConfig;
use migratus::machine::builder::MigrationBuilder;
use std::fs;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Detect binary name (migratus vs updatus)
    let binary_name = std::env::args()
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "migratus".to_string());
    
    let is_updatus = binary_name == "updatus";
    
    // Try to parse as new CLI first, fall back to old behavior
    let args: Vec<String> = std::env::args().collect();
    
    // If only 2 args and second is a file path (backward compatibility)
    if args.len() == 2 && !args[1].starts_with('-') && PathBuf::from(&args[1]).exists() {
        // Old behavior: migratus <config.json>
        run_all_stages(&PathBuf::from(&args[1])).await
    } else {
        // New behavior: migratus/updatus <command> <config.json>
        let cli = Cli::parse();
        route_command(cli.command, is_updatus).await
    }
}

async fn route_command(command: Command, is_updatus: bool) -> Result<(), Box<dyn std::error::Error>> {
    // For updatus, only allow update-related commands
    if is_updatus {
        match &command {
            Command::Update { .. } | Command::Load { .. } | Command::Validate { .. } 
            | Command::Enrich { .. } | Command::Batch { .. } | Command::Complete { .. } 
            | Command::Status { .. } => {
                // Allowed for updatus
            }
            Command::Migrate { .. } | Command::Run { .. } => {
                return Err(format!(
                    "Command '{}' not available for updatus. Use 'update' instead of 'migrate'.",
                    match command {
                        Command::Migrate { .. } => "migrate",
                        Command::Run { .. } => "run",
                        _ => "unknown"
                    }
                ).into());
            }
        }
    }
    
    match command {
        Command::Run { config } => run_all_stages(&config).await,
        Command::Load { config } => {
            migratus::cli::commands::handle_load(&config).await
        }
        Command::Validate { config, force } => {
            migratus::cli::commands::handle_validate(&config, force).await
        }
        Command::Enrich { config, force } => {
            migratus::cli::commands::handle_enrich(&config, force).await
        }
        Command::Batch { config, force } => {
            migratus::cli::commands::handle_batch(&config, force).await
        }
        Command::Migrate { config, from_batch, count, all, force } => {
            migratus::cli::commands::handle_migrate(&config, from_batch, count, all, force).await
        }
        Command::Update { config, from_batch, count, all, force } => {
            migratus::cli::commands::handle_update(&config, from_batch, count, all, force).await
        }
        Command::Complete { config, force } => {
            migratus::cli::commands::handle_complete(&config, force).await
        }
        Command::Status { config } => {
            migratus::cli::commands::handle_status(&config).await
        }
    }
}

async fn run_all_stages(config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {

    println!("🚀 Migratus - Data Migration Tool");
    println!("==================================\n");

    // Load configuration
    println!("📄 Loading configuration from: {}", config_path.display());
    let config_json = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let config: MigrationConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("Failed to parse config: {}", e))?;

    println!("✓ Configuration loaded successfully\n");

    // Display configuration summary
    println!("📊 Migration Configuration:");
    println!("  - Data source: {:?}", config.data_source);
    println!("  - Batch size: {}", config.batch_config.batch_size);
    println!("  - Output dir: {:?}", config.output_config.output_dir);
    println!("  - API endpoint: {}", config.api_config.endpoint);
    println!();

    // Initialize migration
    println!("🔧 Initializing migration pipeline...");
    let builder = MigrationBuilder::new(config);
    let decision = builder.initialize()?;
    println!("✓ Initialization complete\n");

    // Step 1: Load/Merge data
    println!("📥 Step 1/7: Loading data...");
    let validated = match decision {
        migratus::machine::builder::BranchDecision::RequiresMerge(b) => {
            println!("  → Merging customer and payment data...");
            b.merge().await?
        }
        migratus::machine::builder::BranchDecision::SkipMerge(b) => {
            println!("  → Loading pre-merged data...");
            b.load_merged_data().await?
        }
    };
    println!("✓ Data loaded\n");

    // Step 2: Validate
    println!("✅ Step 2/7: Validating records...");
    let enriched = validated.validate().await?;
    println!("✓ Validation complete\n");

    // Step 3: Enrich
    println!("➕ Step 3/7: Enriching records...");
    let mut columns = migratus::domain::config::EnrichmentColumns::new();
    
    // Add enrichment columns from config
    if let Some(enrichment) = &enriched.config.enrichment {
        for (key, value) in enrichment {
            columns.add(key.clone(), value.clone());
        }
    }
    
    let batched = enriched.enrich(columns).await?;
    println!("✓ Enrichment complete\n");

    // Step 4: Create batches
    println!("📦 Step 4/7: Creating batches...");
    println!("  → First batch: 10 records");
    println!(
        "  → Remaining batches: {} records each",
        batched.config.batch_config.batch_size
    );
    let migrated = batched.batch().await?;
    println!("✓ Batching complete\n");

    // Step 5: Migrate
    println!("🚀 Step 5/7: Running migration...");
    println!(
        "  ⚠️  Making API calls to: {}",
        migrated.config.api_config.endpoint
    );
    let completed = migrated.migrate().await?;
    println!("✓ Migration complete\n");

    // Step 6: Generate outputs
    println!("📝 Step 6/7: Generating output files...");
    let output = completed.complete().await?;
    println!("✓ Output files generated\n");

    // Step 7: Display summary
    println!("📊 Step 7/7: Migration Summary");
    println!("================================");
    println!(
        "Total input records:       {}",
        output.summary.total_input_records
    );
    println!(
        "Valid for migration:       {}",
        output.summary.valid_for_migration
    );
    println!(
        "Invalid (pre-migration):   {}",
        output.summary.invalid_pre_migration
    );
    println!(
        "Successful migrations:     {}",
        output.summary.successful_migrations
    );
    println!(
        "Failed migrations:         {}",
        output.summary.failed_migrations
    );
    println!(
        "Total output records:      {}",
        output.summary.total_output_records
    );
    println!();
    println!("Invalid breakdown:");
    println!(
        "  - At merge stage:        {}",
        output.summary.invalid_at_merge
    );
    println!(
        "  - At validation stage:   {}",
        output.summary.invalid_at_validation
    );
    println!();

    // Display output files
    println!("📁 Output Files:");
    println!(
        "  ✓ {}/successful_migrations.csv ({} records)",
        output.summary.total_output_records, output.summary.successful_migrations
    );
    println!(
        "  ✓ {}/failed_migrations.csv ({} records)",
        output.summary.total_output_records, output.summary.failed_migrations
    );
    println!(
        "  ✓ {}/invalid_records.csv ({} records)",
        output.summary.total_output_records, output.summary.invalid_pre_migration
    );
    println!(
        "  ✓ {}/batch_responses/*.json (API responses with headers)",
        output.summary.total_output_records
    );
    println!();

    println!("✨ Migration completed successfully!");

    Ok(())
}

fn print_usage() {
    println!("Migratus - Data Migration Tool");
    println!();
    println!("USAGE:");
    println!("    migratus <config.json>");
    println!();
    println!("ARGUMENTS:");
    println!("    <config.json>    Path to migration configuration file");
    println!();
    println!("EXAMPLE:");
    println!("    migratus config/migration.json");
    println!();
    println!("For more information, see: https://github.com/yourorg/migratus");
}
