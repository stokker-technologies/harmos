# CI Runner Routing

## Why

The migrated pipeline spends excessive time transferring artifacts and serializes lightweight checks on local runners. The user approved project-owned per-job runner routing and two local jobs per provider.

## What Changes

- Adopt the Hub runner field in existing ci.jobs entries, preserving bootstrap, cache and timeout settings.
- Route platform-specific Rust checks, Product/Tests builds and Rust test lanes to the matching local label; retain hosted defaults for lightweight checks and packaging.
- Keep benchmark builds and execution on the local Linux runner, and keep ordinary validation separate from requested standalone benchmarks.
- Render the canonical transfer optimization and retire generated provider-variable routing.
- Replace the existing archived specification Purpose placeholder with its concrete current contract.

## Impact

Only Harmos CI configuration, managed workflow/tooling/guidance copies and maintained CI specification are in scope. Preserve all product behavior, platforms, gates and existing proof. Do not edit the unrelated reduce-compile-time change. Use the existing PR into dev.
