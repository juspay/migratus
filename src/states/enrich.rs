use crate::domain::config::EnrichmentColumns;
use crate::error::*;
use crate::machine::builder::*;
use crate::states::*;

impl MigrationBuilder<Enriched> {
    pub async fn enrich(mut self, columns: EnrichmentColumns) -> Result<MigrationBuilder<Batched>> {
        let mut enriched_data = self
            .enriched_data
            .take()
            .ok_or_else(|| MigrationError::InternalError("No enriched data".to_string()))?;

        for record in &mut enriched_data {
            for (name, value) in columns.iter() {
                record.data.insert(name.clone(), value.clone());
            }
        }

        let mut builder = self.transition_to::<Batched>();
        builder.enriched_data = Some(enriched_data);

        Ok(builder)
    }
}
