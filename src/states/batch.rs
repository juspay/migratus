use crate::domain::records::Batch;
use crate::domain::types::BatchNumber;
use crate::error::*;
use crate::machine::builder::*;
use crate::states::*;

impl MigrationBuilder<Batched> {
    pub async fn batch(mut self) -> Result<MigrationBuilder<Migrated>> {
        let enriched_data = self
            .enriched_data
            .take()
            .ok_or_else(|| MigrationError::InternalError("No enriched data".to_string()))?;

        let regular_batch_size = self
            .config
            .batch_config
            .batch_size()
            .map_err(MigrationError::BatchError)?
            .value();

        let mut batches = Vec::new();
        let first_batch_size = 10; // First batch is always 10 records

        if enriched_data.is_empty() {
            return Err(MigrationError::BatchError(
                "No records to batch".to_string(),
            ));
        }

        // Create first batch (up to 10 records)
        let first_chunk_end = first_batch_size.min(enriched_data.len());
        let first_chunk = enriched_data[..first_chunk_end].to_vec();
        batches.push(Batch::new(BatchNumber::new(1), first_chunk));

        // Create remaining batches with regular batch size
        if enriched_data.len() > first_batch_size {
            for (i, chunk) in enriched_data[first_batch_size..]
                .chunks(regular_batch_size)
                .enumerate()
            {
                batches.push(Batch::new(BatchNumber::new(i + 2), chunk.to_vec()));
            }
        }

        let mut builder = self.transition_to::<Migrated>();
        builder.batches = Some(batches);

        Ok(builder)
    }
}
