pub mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "migratus")]
#[command(about = "Data migration tool for HyperSwitch", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Load and merge input data
    Load {
        /// Path to configuration file
        config: PathBuf,
    },
    
    /// Validate merged records
    Validate {
        /// Path to configuration file
        config: PathBuf,
        
        /// Force validation even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Enrich validated records with additional columns
    Enrich {
        /// Path to configuration file
        config: PathBuf,
        
        /// Force enrichment even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Split enriched records into batches
    Batch {
        /// Path to configuration file
        config: PathBuf,
        
        /// Force batching even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Migrate batches via API
    Migrate {
        /// Path to configuration file
        config: PathBuf,
        
        /// Start from specific batch number
        #[arg(long)]
        from_batch: Option<usize>,
        
        /// Number of batches to migrate (default: 10)
        #[arg(long, default_value = "10")]
        count: usize,
        
        /// Migrate all remaining batches
        #[arg(long)]
        all: bool,
        
        /// Force migration even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Update payment methods via API
    Update {
        /// Path to configuration file
        config: PathBuf,
        
        /// Start from specific batch number
        #[arg(long)]
        from_batch: Option<usize>,
        
        /// Number of batches to update (default: 10)
        #[arg(long, default_value = "10")]
        count: usize,
        
        /// Update all remaining batches
        #[arg(long)]
        all: bool,
        
        /// Force update even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Generate final output files from migration results
    Complete {
        /// Path to configuration file
        config: PathBuf,
        
        /// Force completion even if config hash doesn't match
        #[arg(long)]
        force: bool,
    },
    
    /// Run all stages sequentially
    Run {
        /// Path to configuration file
        config: PathBuf,
    },
    
    /// Show current pipeline status
    Status {
        /// Path to configuration file
        config: PathBuf,
    },
}

impl Command {
    pub fn config_path(&self) -> &PathBuf {
        match self {
            Command::Load { config } => config,
            Command::Validate { config, .. } => config,
            Command::Enrich { config, .. } => config,
            Command::Batch { config, .. } => config,
            Command::Migrate { config, .. } => config,
            Command::Update { config, .. } => config,
            Command::Complete { config, .. } => config,
            Command::Run { config } => config,
            Command::Status { config } => config,
        }
    }
}
