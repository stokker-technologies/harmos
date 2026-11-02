# Startup Error Root Causes

## Why

Startup configuration aggregates render Debug and hide typed refusal sources from carriers. Dependent missing resources must be distinguished from independent failures.

## What Changes

- Keep startup classification and resource/operation fields in the startup owner.
- Classify MissingResource errors as consequences when the same prepare pass has a construction failure, without name matching.
- Render readable root prose and carry complete structured diagnostics.

## Impact

Producer for Dokime ../dokime.wt/rootcause (`fix/plugin-start-root-cause`), developed with an integration-only local Cargo patch. Commit locally only; published revision adoption is deferred to the orchestrator.
