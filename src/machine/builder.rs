use crate::domain::{config::*, records::*};
use crate::error::*;
use crate::states::*;
use std::marker::PhantomData;

pub struct MigrationBuilder<S: State> {
    _state: PhantomData<S>,
    pub config: MigrationConfig,
    pub valid_records: Vec<Record>,
    pub invalid_records: Vec<InvalidRecord>,
    pub merged_data: Option<Vec<MergedRecord>>,
    pub enriched_data: Option<Vec<EnrichedRecord>>,
    pub batches: Option<Vec<Batch>>,
    pub migration_results: Option<MigrationResults>,
    pub original_input_count: usize,
}

impl MigrationBuilder<Uninitialized> {
    pub fn new(config: MigrationConfig) -> Self {
        Self {
            _state: PhantomData,
            config,
            valid_records: Vec::new(),
            invalid_records: Vec::new(),
            merged_data: None,
            enriched_data: None,
            batches: None,
            migration_results: None,
            original_input_count: 0,
        }
    }
}

pub enum BranchDecision {
    RequiresMerge(MigrationBuilder<MergeRequired>),
    SkipMerge(MigrationBuilder<MergeSkipped>),
}

impl<S: State> MigrationBuilder<S> {
    pub fn transition_to<T: State>(self) -> MigrationBuilder<T>
    where
        S: TransitionTo<T>,
    {
        MigrationBuilder {
            _state: PhantomData,
            config: self.config,
            valid_records: self.valid_records,
            invalid_records: self.invalid_records,
            merged_data: self.merged_data,
            enriched_data: self.enriched_data,
            batches: self.batches,
            migration_results: self.migration_results,
            original_input_count: self.original_input_count,
        }
    }

    pub fn set_original_count(&mut self, count: usize) {
        self.original_input_count = count;
    }

    pub fn verify_invariant(&self, current_valid: usize, current_invalid: usize) -> Result<()> {
        let total = current_valid + current_invalid;
        if self.original_input_count != total {
            return Err(MigrationError::RecordCountMismatch {
                expected: self.original_input_count,
                actual: total,
            });
        }
        Ok(())
    }
}

pub struct MergeResult {
    pub merged: Vec<MergedRecord>,
    pub invalid: Vec<InvalidRecord>,
}

pub struct ValidationResult {
    pub valid: Vec<EnrichedRecord>,
    pub invalid: Vec<InvalidRecord>,
}

pub struct BatchResult {
    pub batches: Vec<Batch>,
}

pub struct FinalOutput {
    pub successful_migrations: Vec<SuccessfulMigration>,
    pub failed_migrations: Vec<FailedMigration>,
    pub invalid_records: Vec<InvalidRecord>,
    pub summary: MigrationSummary,
}
