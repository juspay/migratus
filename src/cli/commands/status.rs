use crate::domain::config::MigrationConfig;
use crate::utils::state::{
    get_completed_batches, get_total_batches, infer_current_stage, PipelineStage,
};
use std::fs;
use std::path::Path;

pub async fn handle_status(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Load config to get output directory
    let config_json = fs::read_to_string(config_path)?;
    let config: MigrationConfig = serde_json::from_str(&config_json)?;
    let output_dir = &config.output_config.output_dir;

    // Infer current stage
    let current_stage = infer_current_stage(output_dir);

    println!("📊 Pipeline Status");
    println!("==================\n");

    // Show current stage
    println!("Current Stage: {}", current_stage.as_str());
    println!();

    // Show progress for each stage
    println!("Progress:");
    show_stage_status(output_dir, PipelineStage::Load, &current_stage);
    show_stage_status(output_dir, PipelineStage::Validate, &current_stage);
    show_stage_status(output_dir, PipelineStage::Enrich, &current_stage);
    show_stage_status(output_dir, PipelineStage::Batch, &current_stage);
    show_stage_status(output_dir, PipelineStage::Migrate, &current_stage);
    show_stage_status(output_dir, PipelineStage::Complete, &current_stage);

    // Show next action
    println!();
    if current_stage == PipelineStage::Done {
        println!("✅ All stages complete!");
    } else {
        println!("Next Action:");
        println!(
            "  migratus {} {}",
            current_stage.as_str().to_lowercase(),
            config_path.display()
        );
    }

    Ok(())
}

fn show_stage_status(output_dir: &Path, stage: PipelineStage, current: &PipelineStage) {
    let symbol = if is_stage_complete(output_dir, stage) {
        "✓"
    } else if stage == *current {
        "⏳"
    } else {
        "⏸"
    };

    let mut status_line = format!("  {} {}", symbol, stage.as_str());

    // Add details for specific stages
    match stage {
        PipelineStage::Load => {
            if output_dir.join("merged_records.json").exists() {
                if let Ok(content) = fs::read_to_string(output_dir.join("merged_records.json")) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(count) = json.get("record_count").and_then(|v| v.as_u64()) {
                            status_line.push_str(&format!(" - {} records", count));
                        }
                    }
                }
            }
        }
        PipelineStage::Validate => {
            if output_dir.join("validated_records.json").exists() {
                if let Ok(content) = fs::read_to_string(output_dir.join("validated_records.json")) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(count) = json.get("record_count").and_then(|v| v.as_u64()) {
                            status_line.push_str(&format!(" - {} valid", count));
                        }
                    }
                }
            }
        }
        PipelineStage::Batch => {
            let total = get_total_batches(output_dir);
            if total > 0 {
                status_line.push_str(&format!(" - {} batches created", total));
            }
        }
        PipelineStage::Migrate => {
            let total = get_total_batches(output_dir);
            let completed = get_completed_batches(output_dir);
            if total > 0 {
                status_line.push_str(&format!(
                    " - {}/{} batches migrated ({}%)",
                    completed.len(),
                    total,
                    (completed.len() * 100) / total
                ));
            }
        }
        _ => {}
    }

    println!("{}", status_line);
}

fn is_stage_complete(output_dir: &Path, stage: PipelineStage) -> bool {
    match stage {
        PipelineStage::Load => output_dir.join("merged_records.json").exists(),
        PipelineStage::Validate => output_dir.join("validated_records.json").exists(),
        PipelineStage::Enrich => output_dir.join("enriched_records.json").exists(),
        PipelineStage::Batch => {
            let batches_dir = output_dir.join("batches");
            batches_dir.exists() && get_total_batches(output_dir) > 0
        }
        PipelineStage::Migrate => {
            let total = get_total_batches(output_dir);
            let completed = get_completed_batches(output_dir);
            total > 0 && completed.len() == total
        }
        PipelineStage::Complete => output_dir.join("summary.json").exists(),
        PipelineStage::Done => output_dir.join("summary.json").exists(),
    }
}
