# Runtime Observer Cadence

## Why

Dokime needs every host stream cadence to come from resolved plugin settings.
Harmos currently compiles cadence into observer metadata and its scheduler.
The owner approved this paired change, commits only and no push.

## What Changes

- Retain required observer defaults and allow optional resource-based activation overrides.
- Remove cadence from static observer inventories and backend definitions.
- Resolve and validate cadence once per activation before initializing and sampling.
- Migrate native and sidecar consumers, macros, examples, templates, tests and docs.

## Impact

This is a clean SDK break. Schema, stream replay windows, observer identities,
activation and demand semantics remain static. Operation policies are unchanged.
