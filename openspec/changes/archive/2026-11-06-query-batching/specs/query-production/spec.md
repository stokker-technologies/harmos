## ADDED Requirements

### Requirement: Bounded Exact Numeric Spooling

Finite-query consumers SHALL be able to spool finite f64 projections for shared exact linear percentiles without decoding source evidence again. Payload, spill buffering and rank scratch SHALL fit caller-supplied memory budgets. Selection SHALL poll a caller-supplied cancellation check and release temporary storage on return. Invalid inputs or budget exhaustion SHALL fail without approximation or partial results.

#### Scenario: Memory And Disk Parity

- **WHEN** finite numeric projections cross their memory allocation
- **THEN** exact percentile bits agree with independently sorted values, including signed zero, subnormals, duplicates and opposite finite extremes

#### Scenario: Working Memory Boundary

- **WHEN** numeric payloads spill or rank selection allocates scratch
- **THEN** the retained payload and writer buffer fit the payload budget even during the transition
- **AND** rank counters, results and read buffers fit the scratch budget

#### Scenario: Invalid Numeric Input

- **WHEN** a value is non-finite or a requested percentile lies outside 0..=100
- **THEN** the spool rejects the input

#### Scenario: Cancelled Or Exhausted Selection

- **WHEN** the consumer check refuses further work or rank scratch is insufficient
- **THEN** selection returns an error without a partial result and releases its temporary storage
