# Migratus

A robust, stage-by-stage data migration and update tool for HyperSwitch payment method operations.

## 🎯 Overview

Migratus provides two complete pipelines for payment method operations:
- **Migration Flow** (`migratus`): Migrate payment methods from external systems
- **Update Flow** (`updatus`): Update existing payment methods in HyperSwitch

### Key Features

✅ **Stage-by-Stage Execution** - Run the entire pipeline or individual stages  
✅ **Resume Capabilities** - Auto-resume from failures or run specific batches  
✅ **Type-Safe Operations** - Compile-time safety with Rust's type system  
✅ **Flexible Data Sources** - Support for merged or separate customer/payment files  
✅ **Comprehensive Validation** - Flow-based validation with detailed error tracking  
✅ **Batch Processing** - Configurable batch sizes with progress tracking  
✅ **Complete Audit Trail** - All responses and errors tracked with metadata  
✅ **Config Hash Verification** - Prevents inconsistent pipeline runs  

## 🚀 Quick Start

### Installation

```bash
# Build both binaries
cargo build --release

# Binaries will be in target/release/
# - migratus (for migrations)
# - updatus (for updates)
```

### Migration Flow - Quick Example

```bash
# Run complete migration pipeline
migratus run config/migration.json

# Or run stage-by-stage
migratus load config/migration.json
migratus validate config/migration.json
migratus enrich config/migration.json
migratus batch config/migration.json
migratus migrate config/migration.json --count 10
migratus complete config/migration.json

# Check status anytime
migratus status config/migration.json
```

### Update Flow - Quick Example

```bash
# Run complete update pipeline
updatus run config/update.json

# Or run stage-by-stage
updatus load config/update.json
updatus validate config/update.json
updatus enrich config/update.json
updatus batch config/update.json
updatus update config/update.json --count 10
updatus complete config/update.json

# Check status anytime
updatus status config/update.json
```

## 📋 Pipeline Stages

Both flows follow the same 6-stage pipeline:

1. **LOAD** - Load and merge input data
2. **VALIDATE** - Validate and filter records (duplicates, required fields)
3. **ENRICH** - Add additional columns (e.g., merchant_id, timestamps)
4. **BATCH** - Split into manageable CSV batches
5. **MIGRATE/UPDATE** - Execute API calls with resume support
6. **COMPLETE** - Generate final output CSVs and summary

## 🎛️ CLI Commands

### Common Commands (Both Flows)

```bash
# Stage-by-stage execution
<binary> load <config>         # Load/merge data
<binary> validate <config>     # Validate records
<binary> enrich <config>       # Add enrichment columns
<binary> batch <config>        # Create batches
<binary> complete <config>     # Generate final outputs
<binary> status <config>       # Show pipeline status

# Full pipeline execution
<binary> run <config>          # Run all stages

# Migration-specific
migratus migrate <config> [OPTIONS]

# Update-specific
updatus update <config> [OPTIONS]
```

### Migrate/Update Command Options

```bash
# Resume from specific batch
migratus migrate config.json --from-batch 5

# Process limited number of batches
migratus migrate config.json --count 10

# Process all remaining batches
migratus migrate config.json --all

# Force run despite config hash mismatch
migratus migrate config.json --force
```

## ⚙️ Configuration

### Minimal Migration Config

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
    "merchant_id": "merchant_123",
    "merchant_connector_ids": ["mca_456"],
    "timeout_secs": 300
  },
  "batch_config": {
    "batch_size": 100
  },
  "output_config": {
    "output_dir": "output"
  }
}
```

### Minimal Update Config

```json
{
  "flow": {
    "type": "update"
  },
  "data_source": {
    "type": "merged",
    "path": "input/payment_methods.csv"
  },
  "api_config": {
    "endpoint": "https://api.hyperswitch.io/payment_methods/update-batch",
    "api_key": "YOUR_API_KEY",
    "merchant_id": "merchant_123",
    "merchant_connector_ids": ["mca_456"],
    "timeout_secs": 300
  },
  "batch_config": {
    "batch_size": 100
  },
  "output_config": {
    "output_dir": "output"
  }
}
```

## 📤 Output Files

### Migration Flow

- `merged_records.json` - Loaded data with metadata
- `validated_records.json` - Valid records after validation
- `enriched_records.json` - Records with enrichment columns
- `batches/batch_*.csv` - Numbered batch files
- `batch_responses/batch_*.json` - API responses with headers
- `successful_migrations.csv` - Successfully migrated records
- `failed_migrations.csv` - Failed migrations with error details
- `invalid_records.csv` - Invalid records from all stages
- `summary.json` - Final statistics

### Update Flow

Same structure, but with:
- `successful_updates.csv` instead of successful_migrations.csv
- `failed_updates.csv` instead of failed_migrations.csv

## 🔄 Resume Capabilities

### Automatic Resume

```bash
# Automatically resumes from last completed batch
migratus migrate config.json
```

### Manual Resume

```bash
# Resume from specific batch
migratus migrate config.json --from-batch 25

# Process next 10 batches
migratus migrate config.json --from-batch 25 --count 10

# Process all remaining
migratus migrate config.json --all
```

### Error Recovery

When a batch fails:
```
❌ Batch 15 failed with status 400: Invalid card data
   
⚠️  Migration halted. Fix the issue and retry with:
  migratus migrate config.json --from-batch 15
```

## 📊 Status Monitoring

```bash
migratus status config.json
```

Output:
```
📊 Pipeline Status
==================

Current Stage: MIGRATE

Progress:
  ✓ LOAD - 1000 records
  ✓ VALIDATE - 950 valid
  ✓ ENRICH
  ✓ BATCH - 10 batches created
  ⏳ MIGRATE - 5/10 batches migrated (50%)
  ⏸ COMPLETE

Next Action:
  migratus migrate config.json
```

## 🎭 Migration Flows

### RawCard Flow
Migrate raw card data from external systems:
```json
{
  "flow": {
    "type": "raw_card"
  }
}
```

**Required Fields**: customer_id, raw_card_number, card_expiry_month, card_expiry_year  
**Optional Fields**: name, email, billing address fields, etc.

### PSP Token Flow
Migrate PSP tokens:
```json
{
  "flow": {
    "type": "psp_token"
  }
}
```

**Required Fields**: customer_id, payment_instrument_id, card_number_masked, card_expiry_month, card_expiry_year  
**Optional Fields**: connector_customer_id, etc.

### Update Flow
Update existing payment methods:
```json
{
  "flow": {
    "type": "update"
  }
}
```

**Required Fields**: payment_method_id, any fields to update

### Custom Flow
Define your own required/optional fields:
```json
{
  "flow": {
    "type": "custom",
    "required_fields": ["customer_id", "card_number_masked"],
    "optional_fields": ["email", "name"]
  }
}
```

## 🛠️ Advanced Features

### Data Sources

**Merged Data** (single CSV):
```json
{
  "data_source": {
    "type": "merged",
    "path": "data.csv"
  }
}
```

**Separate Files** (auto-merge):
```json
{
  "data_source": {
    "type": "separate",
    "customer": "customers.csv",
    "payment": "payments.csv",
    "merge_on": "customer_id"
  }
}
```

### Enrichment

Add columns to all records:
```json
{
  "enrichment": {
    "merchant_id": "merchant_123",
    "source_system": "legacy_psp",
    "migration_date": "2025-01-15"
  }
}
```

### Custom Output Fields

Select specific fields for output CSV:
```json
{
  "output_config": {
    "output_dir": "output",
    "output_fields": [
      "customer_id",
      "payment_method_id",
      "card_number_masked",
      "migration_status",
      "card_migrated"
    ]
  }
}
```

### Force Flags

Override safety checks when needed:
```bash
# Ignore config hash mismatch
migratus validate config.json --force
migratus migrate config.json --force
```

## 📚 Documentation

- **[Comprehensive Usage Guide](docs/USAGE.md)** - Detailed documentation for all features
- **[Configuration Reference](docs/USAGE.md#configuration)** - Complete config file guide
- **[API Integration](docs/USAGE.md#api-integration)** - API endpoint details
- **[Troubleshooting](docs/USAGE.md#troubleshooting)** - Common issues and solutions

## 🧪 Development

```bash
# Build
cargo build

# Run with example config
cargo run --release -- run examples/config.json

# Run tests (when available)
cargo test

# Check code
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

## 🏗️ Architecture

### Type-Safe State Machine (Legacy)

The original state machine is still used internally for the `run` command:

```
Uninitialized → [Merge/Load] → Validated → Enriched → Batched → Migrated → Completed
```

### Modern CLI Architecture

Individual commands bypass the state machine for flexibility:
- Direct operations for each stage
- Independent execution
- Config hash validation
- State inference from output files

## 🤝 Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests (when test infrastructure is ready)
5. Submit a pull request

## 📝 License

[Your License Here]

## 🆘 Support

For issues, questions, or feature requests, please open an issue on the repository.

---

**Note**: This tool is designed for HyperSwitch payment method operations. Ensure you have proper authorization and understand the implications before running migrations or updates on production data.
