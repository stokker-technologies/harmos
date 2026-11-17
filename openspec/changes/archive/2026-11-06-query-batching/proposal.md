# Query Batching

## Why

Adopt the measured batch-decoding and numeric-spooling control on current dev, preserving exact query results and policy limits without adopting a query engine.

## What Changes

Harmos provides bounded exact numeric spooling with cooperative checks. a product folds projected Arrow columns for statistics and buckets, preserving sequential arithmetic and selection semantics.

## Impact

Harmos owns its storage query spool and integration tests. a product owns runtime query adaptation and telemetry batch admission. Harmos merges first; a product pins the merged revision without local overrides. No dependency additions, Arrow downgrades, measurement switches or duplicate benches.
