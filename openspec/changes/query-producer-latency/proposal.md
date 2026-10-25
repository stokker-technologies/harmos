# Query Producer Latency

## Why

Dokime's durable Stop acknowledgement exceeds its existing RPC deadline while waiting for small native telemetry query results. Consumer measurements attribute most of the delay to waiting for producer batches rather than decoding or report statistics. The exact upstream cause must be measured before changing behavior.

## What Changes

- Attribute storage scan, conversion, query planning and batch delivery with temporary aggregate diagnostics.
- Repair the demonstrated cause and add a deterministic regression at its owning boundary.
- Preserve storage validation, bounded reservations, cancellation, ordering, exact values and all deadlines.
- Exercise the paired Dokime consumer through an untracked Cargo patch; remove diagnostics and publish the validated upstream before final consumer adoption.

## Capabilities

### New Capabilities

- `query-production`: Correct and bounded native query production through durable consumer completion.

### Modified Capabilities

None.

## Impact

Owned upstream workspace: harmos.wt/query-producer, branch fix/query-producer, based on published 0.1.4. Paired consumer: dokime.wt/hub-ci and its embedded-server-and-ci change. Initial inventory is storage query reader and runtime query handler diagnostics; behavioral source and regression inventory will be locked after attribution. No public API, format, compiler-profile or deadline change is authorized.
