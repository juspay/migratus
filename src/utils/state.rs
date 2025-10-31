use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Load,
    Validate,
    Enrich,
    Batch,
    Migrate,
    Complete,
    Done,
}

impl PipelineStage {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Load => "LOAD",
            Self::Validate => "VALIDATE",
            Self::Enrich => "ENRICH",
            Self::Batch => "BATCH",
            Self::Migrate => "MIGRATE",
            Self::Complete => "COMPLETE",
            Self::Done => "DONE",
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Load => Some(Self::Validate),
            Self::Validate => Some(Self::Enrich),
            Self::Enrich => Some(Self::Batch),
            Self::Batch => Some(Self::Migrate),
            Self::Migrate => Some(Self::Complete),
            Self::Complete => Some(Self::Done),
            Self::Done => None,
        }
    }
}

/// Infer the current pipeline stage based on existing output files
pub fn infer_current_stage(output_dir: &Path) -> PipelineStage {
    // Check files in reverse order (most advanced stage first)
    
    // Check if summary.json exists (DONE)
    if output_dir.join("summary.json").exists() {
        return PipelineStage::Done;
    }
    
    // Check if batch_responses directory has files (COMPLETE ready)
    let batch_responses_dir = output_dir.join("batch_responses");
    if batch_responses_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&batch_responses_dir) {
            if entries.count() > 0 {
                return PipelineStage::Complete;
            }
        }
    }
    
    // Check if batches directory has files (MIGRATE ready)
    let batches_dir = output_dir.join("batches");
    if batches_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&batches_dir) {
            if entries.count() > 0 {
                return PipelineStage::Migrate;
            }
        }
    }
    
    // Check if enriched_records.json exists (BATCH ready)
    if output_dir.join("enriched_records.json").exists() {
        return PipelineStage::Batch;
    }
    
    // Check if validated_records.json exists (ENRICH ready)
    if output_dir.join("validated_records.json").exists() {
        return PipelineStage::Enrich;
    }
    
    // Check if merged_records.json exists (VALIDATE ready)
    if output_dir.join("merged_records.json").exists() {
        return PipelineStage::Validate;
    }
    
    // Nothing exists - need to LOAD
    PipelineStage::Load
}

/// Get list of completed batch numbers by checking batch_responses directory
pub fn get_completed_batches(output_dir: &Path) -> Vec<usize> {
    let batch_responses_dir = output_dir.join("batch_responses");
    let mut completed = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&batch_responses_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                // Parse batch_0001.json -> 1
                if filename.starts_with("batch_") && filename.ends_with(".json") {
                    let num_str = &filename[6..filename.len() - 5]; // Extract "0001"
                    if let Ok(num) = num_str.parse::<usize>() {
                        completed.push(num);
                    }
                }
            }
        }
    }
    
    completed.sort();
    completed
}

/// Get total number of batches by checking batches directory
pub fn get_total_batches(output_dir: &Path) -> usize {
    let batches_dir = output_dir.join("batches");
    
    if let Ok(entries) = std::fs::read_dir(&batches_dir) {
        entries
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("batch_") && s.ends_with(".csv"))
                    .unwrap_or(false)
            })
            .count()
    } else {
        0
    }
}

/// Get the next batch number to migrate (first missing batch)
pub fn get_next_batch_to_migrate(output_dir: &Path) -> usize {
    let completed = get_completed_batches(output_dir);
    let total = get_total_batches(output_dir);
    
    if completed.is_empty() {
        return 1; // Start from batch 1
    }
    
    // Find first missing batch
    for i in 1..=total {
        if !completed.contains(&i) {
            return i;
        }
    }
    
    // All batches completed
    total + 1
}

/// Get paths for intermediate JSON files
pub struct IntermediatePaths {
    pub merged_records: PathBuf,
    pub validated_records: PathBuf,
    pub enriched_records: PathBuf,
}

impl IntermediatePaths {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            merged_records: output_dir.join("merged_records.json"),
            validated_records: output_dir.join("validated_records.json"),
            enriched_records: output_dir.join("enriched_records.json"),
        }
    }
}
