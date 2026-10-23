# fixture-transfers Specification

## Purpose
Keep compilation fixtures complete while avoiding repeated transfer of standalone metadata already embedded in a retained matching library.

## Requirements

### Requirement: Preserve Compilation Fixtures Without Duplicate Metadata

The Tests archive SHALL preserve the inputs required by its authored compilation fixtures while omitting standalone metadata already supplied by a retained matching Rust library.

#### Scenario: Redundant Metadata Pair

- **WHEN** staging identifies standalone metadata and a matching retained library that supplies that metadata
- **THEN** the archive omits the standalone duplicate and retains the library

#### Scenario: Unmatched Inputs

- **WHEN** metadata has no matching retained library or an artifact is a required library variant or shared library
- **THEN** staging retains that input

#### Scenario: Real Fixture Consumers

- **WHEN** the two macro compilation consumers run against newly staged artifacts
- **THEN** both retain their existing successful and expected-refusal behavior without downloading replacement fixture inputs
