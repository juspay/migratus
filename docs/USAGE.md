# Migratus/Updatus - Comprehensive Usage Guide

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Two Execution Modes](#two-execution-modes)
4. [Migration Flow](#migration-flow)
5. [Update Flow](#update-flow)
6. [Configuration Reference](#configuration-reference)
7. [Command Reference](#command-reference)
8. [Resume and Recovery](#resume-and-recovery)
9. [Troubleshooting](#troubleshooting)
10. [Best Practices](#best-practices)

---

## Overview

Migratus provides two complete pipelines for HyperSwitch payment method operations:

- **Migration Flow** (`migratus`): Migrate payment methods from external systems to HyperSwitch
- **Update Flow** (`updatus`): Update existing payment methods in HyperSwitch

Both flows follow the same 6-stage pipeline with full resume capabilities.

### Pipeline Stages

1. **LOAD** - Load and optionally merge input data from CSV files
2. **VALIDATE** - Validate records, filter duplicates, check required fields
3. **ENRICH** - Add additional columns to all records
4. **BATCH** - Split records into numbered CSV batches
5. **MIGRATE/UPDATE** - Execute API calls with progress tracking
6. **COMPLETE** - Generate final output files and summary

---

## Installation

### Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd migratus

# Build release binaries
cargo build --release

# Binaries will be in target/release/
ls target/release/migratus
ls target/release/updatus
```

### Optional: Install Globally

```bash
# Install migratus binary
cargo install --path . --bin migratus

# Install updatus binary
cargo install --path . --bin updatus

# Now you can run from anywhere
migratus run config.json
updatus run config.json
```

---

## Two Execution Modes

### Mode 1: Full Pipeline (`run` command)

Execute all stages in a single run:

```bash
# Migration
migratus run config/migration.json

# Update
updatus run config/update.json
```

**Pros:**
- Simple, one command
- Automatic stage transitions
- Good for initial runs

**Cons:**
- Must complete all stages
- Harder to debug individual stages
- Cannot skip stages

### Mode 2: Stage-by-Stage

Execute individual stages for fine-grained control:

```bash
# Migration flow
migratus load config.json
migratus validate config.json
migratus enrich config.json
migratus batch config.json
migratus migrate config.json --count 10
migratus complete config.json

# Update flow
updatus load config.json
updatus validate config.json
updatus enrich config.json
updatus batch config.json
updatus update config.json --count 10
updatus complete config.json
```

**Pros:**
- Fine-grained control
- Easy to debug each stage
- Can re-run individual stages with `--force`
- Better for development/testing

**Cons:**
- More commands to type
- Manual stage progression

---

## Migration Flow

### When to Use

Use the migration flow when:
- Importing payment methods from external systems
- Migrating from legacy payment processors
- Onboarding merchants with existing payment data
- Creating new payment methods in HyperSwitch

### Supported Migration Types

#### 1. Raw Card Migration

Migrate raw card data (card numbers in plaintext):

**Config:**
```json
{
  "flow": {
    "type": "raw_card"
  }
}
```

**Required CSV Columns:**
- `customer_id`
- `raw_card_number`
- `card_expiry_month`
- `card_expiry_year`

**Optional Columns:**
- `name`, `email`
- `billing_address_*` fields
- `card_scheme`

#### 2. PSP Token Migration

Migrate PSP tokens (masked cards):

**Config:**
```json
{
  "flow": {
    "type": "psp_token"
  }
}
```

**Required CSV Columns:**
- `customer_id`
- `payment_instrument_id`
- `card_number_masked`
- `card_expiry_month`
- `card_expiry_year`

**Optional Columns:**
- `connector_customer_id`
- `network_token_*` fields

#### 3. Custom Migration

Define your own field requirements:

**Config:**
```json
{
  "flow": {
    "type": "custom",
    "required_fields": [
      "customer_id",
      "card_number_masked",
      "card_expiry_month",
      "card_expiry_year"
    ],
    "optional_fields": [
      "name",
      "email"
    ]
  }
}
```

### Migration Workflow Example

```bash
# 1. Prepare your data
# customer.csv and payment.csv OR merged.csv

# 2. Create configuration
cat > config.json << EOF
{
  "flow": { "type": "raw_card" },
  "data_source": {
    "type": "separate",
    "customer": "customers.csv",
    "payment": "payments.csv",
    "merge_on": "customer_id"
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
EOF

# 3. Option A: Run full pipeline
migratus run config.json

# 3. Option B: Run stage-by-stage
migratus load config.json
migratus validate config.json
migratus enrich config.json
migratus batch config.json
migratus migrate config.json --count 10  # Test with 10 batches first
migratus migrate config.json --all       # Then migrate remaining
migratus complete config.json

# 4. Check results
cat output/summary.json
head output/successful_migrations.csv
head output/failed_migrations.csv
```

---

## Update Flow

### When to Use

Use the update flow when:
- Updating existing payment method metadata
- Correcting payment method information
- Adding missing connector details
- Updating customer associations

### Update Requirements

**Config:**
```json
{
  "flow": {
    "type": "update"
  }
}
```

**Required CSV Columns:**
- `payment_method_id` - The ID of the payment method to update
- Any fields you want to update (e.g., `billing_address_line1`, `email`)

### Update Workflow Example

```bash
# 1. Prepare payment methods to update
cat > payment_methods.csv << EOF
payment_method_id,email,billing_address_line1
pm_abc123,john@example.com,123 Main St
pm_def456,jane@example.com,456 Oak Ave
EOF

# 2. Create configuration
cat > update_config.json << EOF
{
  "flow": { "type": "update" },
  "data_source": {
    "type": "merged",
    "path": "payment_methods.csv"
  },
  "api_config": {
    "endpoint": "https://api.hyperswitch.io/payment_methods/update-batch",
    "api_key": "YOUR_API_KEY",
    "merchant_id": "merchant_123",
    "merchant_connector_ids": ["mca_456"],
    "timeout_secs": 300
  },
  "batch_config": {
    "batch_size": 50
  },
  "output_config": {
    "output_dir": "output"
  }
}
EOF

# 3. Run update pipeline
updatus load update_config.json
updatus validate update_config.json
updatus batch update_config.json
updatus update update_config.json --all
updatus complete update_config.json

# 4. Check results
cat output/summary.json
head output/successful_updates.csv
head output/failed_updates.csv
```

---

## Configuration Reference

### Complete Configuration Template

```json
{
  "flow": {
    "type": "raw_card | psp_token | update | custom",
    "required_fields": ["field1", "field2"],  // Only for custom
    "optional_fields": ["field3"]              // Only for custom
  },
  "data_source": {
    "type": "merged | separate",
    // For merged:
    "path": "data.csv",
    // For separate:
    "customer": "customers.csv",
    "payment": "payments.csv",
    "merge_on": "customer_id"
  },
  "api_config": {
    "endpoint": "https://api.example.com/endpoint",
    "api_key": "your_api_key",
    "merchant_id": "merchant_id",
    "merchant_connector_ids": ["mca_1", "mca_2"],  // Optional array
    "timeout_secs": 300
  },
  "batch_config": {
    "batch_size": 100
  },
  "output_config": {
    "output_dir": "output",
    "output_fields": [         // Optional: customize output CSV fields
      "customer_id",
      "payment_method_id",
      "migration_status"
    ]
  },
  "enrichment": {              // Optional: add columns to all records
    "merchant_id": "value1",
    "custom_field": "value2"
  }
}
```

### Data Source Options

#### Merged Data Source

Use when you have a single CSV with all data:

```json
{
  "data_source": {
    "type": "merged",
    "path": "combined_data.csv"
  }
}
```

**CSV Format:**
```csv
customer_id,raw_card_number,card_expiry_month,card_expiry_year,name,email
cust_1,4111111111111111,12,2025,John Doe,john@example.com
cust_2,4242424242424242,06,2026,Jane Smith,jane@example.com
```

#### Separate Data Source

Use when you have separate customer and payment files:

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

**customers.csv:**
```csv
customer_id,name,email
cust_1,John Doe,john@example.com
cust_2,Jane Smith,jane@example.com
```

**payments.csv:**
```csv
customer_id,raw_card_number,card_expiry_month,card_expiry_year
cust_1,4111111111111111,12,2025
cust_2,4242424242424242,06,2026
```

### Enrichment

Add columns to all records before batching:

```json
{
  "enrichment": {
    "merchant_id": "merchant_123",
    "source_system": "legacy_stripe",
    "migration_batch": "2025-01-15",
    "migration_notes": "Q1 2025 migration"
  }
}
```

These fields will be added to every record in the batches.

### Custom Output Fields

Control which fields appear in the output CSV:

```json
{
  "output_config": {
    "output_dir": "output",
    "output_fields": [
      "customer_id",
      "payment_method_id",
      "card_number_masked",
      "migration_status",
      "card_migrated",
      "network_token_migrated"
    ]
  }
}
```

If not specified, all available fields are included.

---

## Command Reference

### Common Commands

#### load

Load and optionally merge data from CSV files.

```bash
migratus load <config.json>
updatus load <config.json>
```

**Output:**
- `output/merged_records.json` - Loaded records with metadata
- `output/invalid_records.csv` - Records that failed merge (if applicable)

---

#### validate

Validate records and filter duplicates.

```bash
migratus validate <config.json> [--force]
updatus validate <config.json> [--force]
```

**Options:**
- `--force` - Skip config hash verification

**Output:**
- `output/validated_records.json` - Valid records
- `output/invalid_records.csv` - Updated with validation failures

**Validation Checks:**
- Required fields present
- Duplicate detection (customer_id + card_number_masked)
- Field format validation (based on flow type)

---

#### enrich

Add enrichment columns to records.

```bash
migratus enrich <config.json> [--force]
updatus enrich <config.json> [--force]
```

**Options:**
- `--force` - Skip config hash verification

**Output:**
- `output/enriched_records.json` - Records with enrichment columns

If no enrichment is configured, records are copied as-is.

---

#### batch

Split records into numbered CSV batches.

```bash
migratus batch <config.json> [--force]
updatus batch <config.json> [--force]
```

**Options:**
- `--force` - Skip config hash verification

**Output:**
- `output/batches/batch_0001.csv`
- `output/batches/batch_0002.csv`
- ... (numbered sequentially)

---

#### migrate

Execute migration API calls for batches.

```bash
migratus migrate <config.json> [OPTIONS]
```

**Options:**
- `--from-batch <N>` - Start from specific batch number
- `--count <N>` - Process N batches (default: 10)
- `--all` - Process all remaining batches
- `--force` - Skip config hash verification

**Examples:**
```bash
# Migrate first 10 batches
migratus migrate config.json

# Migrate batches 5-15
migratus migrate config.json --from-batch 5 --count 10

# Migrate all remaining batches
migratus migrate config.json --all

# Resume from batch 25
migratus migrate config.json --from-batch 25
```

**Output:**
- `output/batch_responses/batch_NNNN.json` - API responses with headers

**Behavior:**
- Automatically skips already-migrated batches
- Saves response immediately after each batch
- Halts on error with recovery instructions
- Shows progress bar for long-running operations

---

#### update

Execute update API calls for batches.

```bash
updatus update <config.json> [OPTIONS]
```

**Options:** Same as migrate command

**Examples:**
```bash
# Update first 10 batches
updatus update config.json

# Update all batches
updatus update config.json --all
```

---

#### complete

Generate final output files from batch responses.

```bash
migratus complete <config.json> [--force]
updatus complete <config.json> [--force]
```

**Options:**
- `--force` - Skip config hash verification

**Output (Migration):**
- `output/successful_migrations.csv`
- `output/failed_migrations.csv`
- `output/summary.json`

**Output (Update):**
- `output/successful_updates.csv`
- `output/failed_updates.csv`
- `output/summary.json`

---

#### status

Show current pipeline status.

```bash
migratus status <config.json>
updatus status <config.json>
```

**Output:**
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

---

#### run

Execute all stages in sequence.

```bash
migratus run <config.json>
updatus run <config.json>
```

Executes: load → validate → enrich → batch → migrate/update → complete

---

## Resume and Recovery

### Automatic Resume

Migratus automatically detects completed work:

```bash
# This will automatically resume from the next unmigrated batch
migratus migrate config.json
```

### Manual Resume

Resume from a specific batch:

```bash
# Resume from batch 15
migratus migrate config.json --from-batch 15

# Process 20 batches starting from batch 15
migratus migrate config.json --from-batch 15 --count 20
```

### Error Recovery

When a batch fails:

```
❌ Batch 15 failed with status 400
   Error: Invalid card data

⚠️  Migration halted. Fix the issue and retry with:
  migratus migrate config.json --from-batch 15
```

**Recovery Steps:**

1. Check the error message
2. Review the failed batch: `output/batch_responses/batch_0015.json`
3. Fix the issue (update config, fix data, etc.)
4. Resume from failed batch:
   ```bash
   migratus migrate config.json --from-batch 15 --force
   ```

### Re-running Stages

Use `--force` to override config hash checks:

```bash
# Re-run validation after config change
migratus validate config.json --force

# Re-run batch creation
migratus batch config.json --force
```

---

## Troubleshooting

### Config Hash Mismatch

**Error:**
```
Config file has changed since LOAD stage.
Use --force to override or re-run from LOAD
```

**Cause:** Configuration file was modified between stages.

**Solution:**
```bash
# Option 1: Re-run from LOAD
migratus load config.json

# Option 2: Override (if change is safe)
migratus validate config.json --force
```

---

### Missing Input Files

**Error:**
```
Merged records not found. Run 'migratus load config.json' first
```

**Cause:** Trying to run a stage without completing previous stages.

**Solution:**
```bash
# Run missing stages in order
migratus load config.json
migratus validate config.json
```

Or check status:
```bash
migratus status config.json
```

---

### Batch Not Found

**Error:**
```
Batch CSV file not found: batch_0015.csv
```

**Cause:** Batch file was deleted or never created.

**Solution:**
```bash
# Re-create batches
migratus batch config.json --force
```

---

### API Timeout

**Error:**
```
Batch 10 failed: request timed out
```

**Solution:**

1. Increase timeout in config:
   ```json
   {
     "api_config": {
       "timeout_secs": 600
     }
   }
   ```

2. Resume with force:
   ```bash
   migratus migrate config.json --from-batch 10 --force
   ```

---

### Duplicate Records

**Behavior:** Records with duplicate `customer_id + card_number_masked` are automatically filtered during validation.

**Check duplicates:**
```bash
# Look for "duplicate" in invalid records
grep -i duplicate output/invalid_records.csv
```

**First occurrence is kept**, duplicates are marked invalid.

---

### All Batches Already Migrated

**Message:**
```
✅ All batches already migrated!
```

**Meaning:** All batches have response files.

**To re-migrate:**
```bash
# Delete responses and re-run
rm -rf output/batch_responses/*.json
migratus migrate config.json --all
```

---

## Best Practices

### 1. Start Small

```bash
# Test with first batch
migratus migrate config.json --count 1

# Review results
cat output/batch_responses/batch_0001.json

# Then process more
migratus migrate config.json --count 10
```

### 2. Use Status Frequently

```bash
# Check progress
migratus status config.json
```

### 3. Validate Before Full Run

```bash
# Run validation only
migratus load config.json
migratus validate config.json

# Check invalid records
cat output/invalid_records.csv

# Fix issues, then continue
migratus enrich config.json
```

### 4. Monitor Batch Responses

```bash
# Check latest response
cat output/batch_responses/batch_0001.json | jq .

# Count successful in batch
cat output/batch_responses/batch_0001.json | jq '.body[] | select(.migration_status == "Success")' | wc -l
```

### 5. Keep Backups

```bash
# Backup before major operations
cp -r output output.backup.$(date +%Y%m%d_%H%M%S)
```

### 6. Use Enrichment for Tracking

```json
{
  "enrichment": {
    "migration_batch_id": "BATCH_2025_01_15",
    "migration_source": "legacy_stripe",
    "migrated_by": "ops_team"
  }
}
```

### 7. Test with Dry Runs

```bash
# Create batches but don't migrate
migratus load config.json
migratus validate config.json
migratus batch config.json

# Inspect batches before migrating
head output/batches/batch_0001.csv
```

### 8. Monitor Progress

```bash
# Watch migration progress
watch -n 5 'migratus status config.json'
```

### 9. Review Summary

```bash
# After completion
cat output/summary.json | jq .

# Check failure rate
jq -r '(.failed_migrations / .total_migrations * 100)' output/summary.json
```

### 10. Handle Failures Promptly

```bash
# If migration halts, review immediately
cat output/batch_responses/batch_0015.json | jq .

# Check batch CSV
cat output/batches/batch_0015.csv

# Fix and resume
migratus migrate config.json --from-batch 15 --force
```

---

## Output File Reference

### Intermediate Files

| File | Stage | Description |
|------|-------|-------------|
| `merged_records.json` | LOAD | Loaded records with config hash and timestamp |
| `validated_records.json` | VALIDATE | Valid records after validation |
| `enriched_records.json` | ENRICH | Records with enrichment columns |
| `batches/batch_*.csv` | BATCH | Numbered batch CSV files |
| `batch_responses/batch_*.json` | MIGRATE/UPDATE | API responses with headers |

### Final Output Files

| File | Description |
|------|-------------|
| `successful_migrations.csv` | Successfully migrated records |
| `failed_migrations.csv` | Failed migration attempts |
| `successful_updates.csv` | Successfully updated records |
| `failed_updates.csv` | Failed update attempts |
| `invalid_records.csv` | All invalid records from all stages |
| `summary.json` | Final statistics and counts |

### Summary JSON Format

```json
{
  "total_batches": 10,
  "successful_migrations": 950,
  "failed_migrations": 50,
  "total_migrations": 1000,
  "unmatched_records": 0
}
```

---

## Advanced Topics

### Batch Response Format

Each batch response file contains:

```json
{
  "batch_number": 1,
  "headers": {
    "content-type": "application/json",
    "x-request-id": "req_123"
  },
  "body": [
    {
      "line_number": 1,
      "customer_id": "cust_1",
      "payment_method_id": "pm_abc123",
      "migration_status": "Success",
      "card_migrated": true,
      "network_token_migrated": false
    }
  ]
}
```

### Config Hash System

Each intermediate file includes a config hash to prevent inconsistent runs:

```json
{
  "config_hash": "abc123...",
  "timestamp": "2025-01-15T10:30:00Z",
  "record_count": 1000,
  "records": [...]
}
```

If config changes between stages, you must either:
1. Re-run from LOAD
2. Use `--force` flag (if change is safe)

---

For more information or support, please refer to the main README.md or open an issue.
