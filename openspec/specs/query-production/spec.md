# query-production Specification

## Purpose
Defines correct, bounded query delivery from producers and how latency repairs are measured.

## Requirements

### Requirement: Correct Bounded Query Delivery

Native query production SHALL preserve validated storage values, ordering, null semantics, cursor bounds, cancellation and memory reservation ownership. A performance repair SHALL preserve storage refusal checks and canonical compiler profiles rather than relaxing consumer deadlines.

#### Scenario: Durable Consumer Report

- **WHEN** Dokime consumes raw, sorted and downsampled telemetry while completing its existing Stop journey
- **THEN** query production supplies complete validated results without introducing a new deadline, cache or skipped validation

#### Scenario: Invalid Stored Payload

- **WHEN** a query encounters data rejected by existing schema or storage validation
- **THEN** the query retains the existing refusal instead of returning partially trusted values

### Requirement: Measured Repair

The repair SHALL follow phase attribution and retain a deterministic regression for the demonstrated cause. Temporary diagnostics SHALL be removed before final source closure.

#### Scenario: Producer Delay Attribution

- **WHEN** a native query delays the durable consumer acknowledgement
- **THEN** aggregate measurements distinguish scan, conversion, planning and delivery without recording payloads or credentials
