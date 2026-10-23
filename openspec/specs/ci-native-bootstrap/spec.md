# CI Native Bootstrap Specification

## Purpose
Give native CI jobs canonical prerequisites, isolated tooling and retained failure diagnostics on hosted and local runners.

## Requirements

### Requirement: Canonical Job Setup
Native CI jobs SHALL obtain declared prerequisites and isolated tooling through canonical CI-owned setup, with long-path checkout support and retained failure diagnostics.

#### Scenario: Native Compiler Job
- **WHEN** a declared compiler job starts on its native hosted or local runner
- **THEN** canonical setup prepares required tools before project commands while preserving its declared routing, bootstrap needs and deadlines

#### Scenario: Concurrent Windows Jobs
- **WHEN** Windows jobs prepare checkout and mise tooling
- **THEN** checkout supports long paths and mise storage uses the system temporary volume with deterministic slot isolation derived from RUNNER_TEMP, without redirecting TEMP or TMP
