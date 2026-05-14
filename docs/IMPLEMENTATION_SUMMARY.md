# Jobs Crate Split - Implementation Summary

## Completed Work

### 1. Created Two New Crates ✅

- **jobs-minidump**: Processes crash minidumps without database access
- **jobs-maintenance**: Handles database imports and maintenance tasks

### 2. Code Migration ✅

- Moved minidump processing logic to jobs-minidump
- Moved maintenance tasks to jobs-maintenance
- Created ImportCrashJob handler for database import
- Split responsibilities cleanly between the two services

### 3. Symbol Supplier Refactoring ✅

- Created simplified S3SymbolSupplier in data crate
- Removed database dependency for symbol lookup
- Falls back to standard Breakpad path structure

### 4. Infrastructure Files ✅

- Created Containerfile.minidump
- Created Containerfile.maintenance
- Documented infrastructure deployment changes in INFRASTRUCTURE_UPDATES.md

### 5. Architecture Changes ✅

- Minidump processor writes results to `processed-crashes/{crash_id}.json`
- Enqueues ImportCrashJob for maintenance worker
- Maintenance worker imports crashes and runs cleanup tasks
- Both services communicate via Apalis job queue

## Remaining Issues

### Compilation Errors

#### jobs-minidump

The main issue is with enqueueing ImportCrashJob to PostgresStorage. The error indicates the `push` method signature needs adjustment.

**Error**: `no method named push found for struct PostgresStorage<Args, Compact, Codec, Fetcher>`

**Solution needed**: The PostgresStorage.push() method requires the Storage trait to be in scope and properly configured. Based on the API crate's usage in worker.rs:

```rust
self.worker
    .clone()
    .push(MinidumpJob { crash })
    .await
```

The fix should be to ensure the Storage trait is properly imported and the generic parameters are correctly specified.

#### jobs-maintenance

Minor syntax or import errors that need resolution.

### Testing Required

Once compilation succeeds:

1. Test minidump processor picks up jobs from queue
2. Verify processed crashes written to S3 `processed-crashes/` folder
3. Verify ImportCrashJob enqueued correctly
4. Test maintenance worker imports crashes into database
5. Verify maintenance tasks run on schedule
6. Check error handling and retry logic

### Infrastructure Deployment

After code works:

1. Copy `apps/guardrail-jobs/` to create:
   - `apps/guardrail-minidump/`
   - `apps/guardrail-maintenance/`
2. Update deployment manifests with correct images and resource limits
3. Create app-of-apps manifests for both services
4. Test in development environment
5. Deploy to production
6. Remove old guardrail-jobs deployment

## Architecture Benefits

✅ **Separation of Concerns**: Minidump processing isolated from database operations
✅ **Independent Scaling**: Can scale minidump processor and maintenance worker separately
✅ **Clearer Dependencies**: Minidump processor has minimal dependencies
✅ **Better Resource Management**: Database connections only in maintenance worker
✅ **Simplified Testing**: Each service can be tested independently
✅ **Communication via Queue**: Loose coupling through Apalis job queue

## Files Created

### New Crates

- `crates/jobs-minidump/`

  - Cargo.toml
  - src/main.rs
  - src/lib.rs
  - src/jobs.rs
  - src/minidump.rs
  - src/signature_generator.rs
  - src/state.rs
  - src/utils.rs

- `crates/jobs-maintenance/`
  - Cargo.toml
  - src/main.rs
  - src/lib.rs
  - src/jobs.rs
  - src/import_crash.rs
  - src/state.rs
  - src/maintenance/ (copied from jobs)

### Infrastructure

- Containerfile.minidump
- Containerfile.maintenance
- INFRASTRUCTURE_UPDATES.md

### Data Crate Enhancement

- data/src/symbol_supplier.rs (simplified S3 symbol loading)

## Next Steps

1. **Fix Compilation Errors** (Priority 1)

   - Fix PostgresStorage.push() usage in jobs-minidump
   - Resolve any remaining syntax errors

2. **Build and Test Locally** (Priority 2)

   - `cargo build --release -p jobs-minidump`
   - `cargo build --release -p jobs-maintenance`
   - Run integration tests

3. **Create Infrastructure** (Priority 3)

   - Follow INFRASTRUCTURE_UPDATES.md guide
   - Create Kubernetes manifests
   - Set up CI/CD pipelines

4. **Deploy to Development** (Priority 4)

   - Test end-to-end flow
   - Monitor logs and metrics
   - Verify queue operations

5. **Production Deployment** (Priority 5)
   - Deploy to production
   - Monitor performance
   - Remove old jobs deployment
