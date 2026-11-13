# Native CI Specification

## Purpose

Define correctness-test deadlines independently of performance measurements.

## Requirements

### Requirement: Correctness Deadlines

Correctness tests SHALL have a minimum 120-second deadline, preserving larger existing bounds. Historical runtimes SHALL NOT tighten correctness deadlines; benchmarks SHALL provide performance evidence independently of test verdicts.

#### Scenario: Correctness Test Configuration

- **WHEN** correctness test deadlines are configured
- **THEN** each deadline is at least 120 seconds and any larger existing bound is preserved
- **AND** benchmarks provide performance evidence independently of correctness verdicts
