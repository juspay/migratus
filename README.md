# Migratus

A type-safe, state-machine-based data migration tool for HyperSwitch payment method migrations.

## Overview

Migratus provides a robust pipeline for migrating payment method data with:
- **Type-safe state machine**: Compile-time guarantees for correct state transitions
- **Flexible data sources**: Support for merged or separate customer/payment files
- **Comprehensive validation**: Flow-based validation with detailed error tracking
- **Batch processing**: Configurable batch sizes with API integration
- **Complete audit trail**: Invalid records tracked at each stage, JSON responses stored

## Architecture

### State Machine Flow

```
Uninitialized
    ↓ initialize()
    ├─→ MergeRequired → merge() → Validated
    └─→ MergeSkipped → load_merged_data() → Validated
                                                ↓ validate()
                                            Enriched
                                                ↓ enrich()
                                            Batched
                                                ↓ batch()
                                            Migrated
                                                ↓ migrate()
                                            Completed
                                                ↓ complete()
                                            FinalOutput
```

### Key Components

**Domain Types** (`src/domain/`)
- `types.rs`: Wrapper types (CustomerId, CardNumber, etc.)
- `config.rs`: Configuration structures
- `records.rs`: Record types for each stage

**State Implementations** (`src/states/`)
- Each state has its own module with transition logic
- Type-safe transitions prevent invalid state changes

**Operations** (`src/operations/`)
- `csv_reader.rs`: CSV file parsing
- `csv_writer.rs`: Output file generation
- `merger.rs`: Customer/payment data merging
- `validator.rs`: Record validation
- `api_client.rs`: Migration API integration

## Usage

### Basic Example

```rust
use migratus::domain::config::*;
use migratus::machine::builder::MigrationBuilder;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MigrationConfig {
        flow: MigrationFlow::RawCard {
            required_fields: vec![
                "customer_id".to_string(),
                "raw_card_number".to_string(),
                "card_expiry_month".to_string(),
                "card_expiry_year".to_string(),
            ],
            optional_fields: vec!["name".to_string(), "email".to_string()],
        },
        data_source: DataSource::Merged {
            path: PathBuf::from("input/data.csv"),
        },
        api_config: ApiConfig {
            endpoint: "https://api.example.com/migrate".to_string(),
            api_key: "your_api_key".to_string(),
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
            batch_response_dir: PathBuf::from("output/responses"),
        },
    };

    let builder = MigrationBuilder::new(config);
    let decision = builder.initialize()?;

    let validated = match decision {
        BranchDecision::RequiresMerge(b) => b.merge().await?,
        BranchDecision::SkipMerge(b) => b.load_merged_data().await?,
    };

    let enriched = validated.validate().await?;
    
    let mut columns = EnrichmentColumns::new();
    columns.add("merchant_id".to_string(), "merchant_123".to_string());
    
    let batched = enriched.enrich(columns).await?;
    let migrated = batched.batch().await?;
    let completed = migrated.migrate().await?;
    let output = completed.complete().await?;

    println!("Migration complete!");
    println!("Successful: {}", output.summary.successful_migrations);
    println!("Failed: {}", output.summary.failed_migrations);

    Ok(())
}
```

## Configuration

### Migration Flows

**RawCard**: Migrate raw card data (fields hardcoded in binary)
```rust
MigrationFlow::RawCard  // No fields needed!
```

**PSPToken**: Migrate PSP tokens (fields hardcoded in binary)
```rust
MigrationFlow::PspToken  // No fields needed!
```

**Custom**: Custom migration flow (specify fields using MigrationField enum)
```rust
use migratus::domain::migration_field::MigrationField as MF;

MigrationFlow::Custom {
    required_fields: vec![
        MF::CustomerId,
        MF::CardNumberMasked,
        MF::CardExpiryMonth,
        MF::CardExpiryYear,
    ],
    optional_fields: vec![
        MF::Name,
        MF::Email,
        MF::BillingAddressLine1,
    ],
}
```

### Output Field Customization

```rust
use migratus::domain::migration_field::MigrationField as MF;

OutputConfig {
    output_dir: PathBuf::from("output"),
    batch_response_dir: PathBuf::from("output/responses"),
    output_fields: Some(vec![
        MF::CustomerId,
        MF::PaymentMethodId,
        MF::CardNumberMasked,
        MF::MigrationStatus,
    ]),
}
```

If `output_fields` is `None`, all fields are included in default format.

### Data Sources

**Merged**: Single CSV file
```rust
DataSource::Merged {
    path: PathBuf::from("data.csv"),
}
```

**Separate**: Customer + Payment files
```rust
DataSource::Separate {
    customer: PathBuf::from("customers.csv"),
    payment: PathBuf::from("payments.csv"),
}
```

## Output Files

Migratus generates three CSV files:

1. **successful_migrations.csv**: Successfully migrated records with payment_method_id
2. **failed_migrations.csv**: Failed migrations with error reasons
3. **invalid_records.csv**: Invalid records from all stages (merge, validation)

Plus JSON responses for each batch in `batch_responses/`.

## Features

### Core Features
- ✅ **Type-safe state machine**: Compile-time safety for state transitions
- ✅ **Flexible data sources**: Merged or separate customer/payment files
- ✅ **Flow-based validation**: RawCard, PSPToken, Custom flows
- ✅ **Record count invariant**: Ensures no data loss
- ✅ **Invalid record tracking**: Captured at each stage with reasons
- ✅ **Configurable batching**: First batch (10 records), then configurable size
- ✅ **API integration**: Timeout handling, retry support
- ✅ **Comprehensive error handling**: Detailed error types and messages
- ✅ **Complete audit trail**: JSON responses with headers stored per batch

### Recent Enhancements (Oct 2025)

#### MigrationField - Single Source of Truth
- **Zero String Duplication**: All 42 field definitions in one enum
- **Type-Safe Throughout**: `Vec<MigrationField>` instead of `Vec<String>`
- **Simplified Configs**: RawCard/PspToken flows don't need field lists
- **Helper Methods**: Type-safe field access via `get_field()` and `has_field()`
- **Production Grade**: Clean, minimal, maintainable code

#### Enhanced Validation
- **Duplicate Detection**: Detects customer_id + card_number_masked duplicates
- **All Duplicates Marked**: First occurrence kept, rest marked invalid
- **Output Field Validation**: Custom output fields validated against CSV headers

#### Customizable Output
- **Custom Output Fields**: Select which fields to include in output CSV
- **42 Predefined Fields**: All API and CSV fields available
- **Custom Fields**: Support for unknown CSV fields via `Custom(String)`
- **Type-Safe Selection**: Uses MigrationField enum for field selection

## Error Handling

All errors are captured in the `MigrationError` enum:
- Configuration errors
- File I/O errors
- CSV parsing errors
- Validation errors
- API errors
- Network errors
- Timeout errors

Invalid records are tracked with:
- Line number
- Original data
- Failure reason
- Stage where failure occurred

## CLI Usage

### Building the Binary

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release
```

### Running Migrations

```bash
# Using the binary
./target/release/migratus path/to/config.json

# Or install globally
cargo install --path .
migratus config.json
```

### Configuration File Format

```json
{
  "flow": {
    "type": "raw_card"
  },
  "data_source": {
    "type": "merged",
    "path": "input/data.csv"
  },
  "api_config": {
    "endpoint": "https://api.hyperswitch.io/payment_methods/migrate-batch",
    "api_key": "YOUR_API_KEY",
    "merchant_id": "YOUR_MERCHANT_ID",
    "merchant_connector_id": "YOUR_MCA_ID",
    "timeout_secs": 300
  },
  "batch_config": {
    "batch_size": 200,
    "resume_from_batch": null
  },
  "output_config": {
    "output_dir": "output",
    "batch_response_dir": "output/batch_responses",
    "output_fields": [
      "customer_id",
      "payment_method_id",
      "card_number_masked",
      "migration_status"
    ]
  }
}
```

## Testing

### Test Scenarios

The `test_scenarios/` directory contains 5 comprehensive test scenarios:

1. **Custom + Separate**: Custom validation with separate files
2. **Raw Card + Separate**: Raw card migration with separate files
3. **Raw Card + Merged**: Raw card migration with pre-merged data
4. **PSP Token + Separate**: PSP token migration with separate files
5. **PSP Token + Merged**: PSP token migration with pre-merged data

Run a test scenario:
```bash
cargo run --release -- test_scenarios/1_custom_separate/config.json
```

See `test_scenarios/README.md` for detailed documentation.

### API Schema Reference

**Required Fields (All Flows)**:
- `customer_id`
- `card_expiry_month`
- `card_expiry_year`

**RawCard Flow**:
- `raw_card_number` (required)
- `card_scheme` (optional)

**PspToken Flow**:
- `payment_instrument_id` (required)
- `card_number_masked` (required)
- `connector_customer_id` (optional)

**All 42 Available Fields**: See `MigrationField` enum in `src/domain/migration_field.rs`

## Development

```bash
# Build
cargo build

# Run example
cargo run --example basic_migration

# Run tests
cargo test

# Check compilation
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy
```

## License

[Your License Here]
