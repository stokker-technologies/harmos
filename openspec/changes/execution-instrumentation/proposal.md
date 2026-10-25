# Execution Instrumentation

## Why

System journeys expose slow commits and storage work without enough attribution to distinguish queueing, execution and durability. Native performance artifacts should explain those paths without a second reporting framework.

## What Changes

- Instrument every operation execution path and storage interface through their shared owners, including queue handoffs and shared persistence work.
- Retain ordinary tracing in release builds; applications own filtering and collection. Add no profiling feature, public subsystem or crate.
- Capture system-test traces with async lifetimes, parent relationships and causal links in a Perfetto-readable native artifact.
- Enable Criterion-native readable reports and retain its native data and baseline comparisons; replace custom benchmark CSV/Markdown summaries at their Hub source owner.
- Preserve payload confidentiality, bounded collection, correctness and existing cancellation/durability behavior.

## Capabilities

### New Capabilities

- `execution-observability`: Structured execution/storage tracing, causal system-test artifacts and native benchmark reports.

### Modified Capabilities

None.

## Impact

Harmos runtime, storage, SDK/carriers, system-test fixtures, Criterion development dependencies, maintained usage documentation and the upstream Hub-generated report runner. Domain data and durable formats remain unchanged. The user authorized implementation, the canonical Hub reinstall and integration of the complete feature into `dev` through required native validation and a ready PR with auto-merge. Hub source remains owned by its existing owner.
