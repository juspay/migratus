use crate::domain::config::DataSource;
use crate::error::*;
use crate::machine::builder::*;
use crate::states::*;

impl MigrationBuilder<Uninitialized> {
    pub fn initialize(self) -> Result<BranchDecision> {
        match &self.config.data_source {
            DataSource::Separate { .. } => {
                let builder = self.transition_to::<MergeRequired>();
                Ok(BranchDecision::RequiresMerge(builder))
            }
            DataSource::Merged { .. } => {
                let builder = self.transition_to::<MergeSkipped>();
                Ok(BranchDecision::SkipMerge(builder))
            }
        }
    }
}
