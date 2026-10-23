# CI Validation Policy Specification

## Purpose
Keep validation caches isolated, cancel superseded ordinary validation, and preserve required native gates and protected release handoffs with two-hour ordinary job budgets.

## Requirements

### Requirement: Scoped Cache Writes
Ordinary GitHub pull-request jobs SHALL save dependency and compiler caches only within the native pull-request merge-ref scope.

#### Scenario: Pull-Request Cache Isolation
- **WHEN** a pull-request merge-ref validation succeeds
- **THEN** its cache writes remain isolated from base branches and other pull requests

### Requirement: Superseded Validation Cancellation
Superseded ordinary pull-request and dev validation SHALL cancel through native concurrency while release preparation, Tag and publication preserve their handoff.

#### Scenario: Release Preparation Handoff
- **WHEN** a release-preparation commit on dev precedes an ordinary dev push
- **THEN** the release-preparation run retains a separate noncancelling identity

### Requirement: Ordinary Job Budget
Ordinary jobs SHALL default to an exact 120-minute whole-job deadline while preserving task deadlines and native required gates.

#### Scenario: Omitted Job Override
- **WHEN** a job has no exceptional timeout override
- **THEN** its rendered whole-job deadline is 120 minutes
