## Purpose

Keep plugin-owned persistent history readable with its written definitions across plugin shape changes and absence, while preserving host state compatibility checks.

## ADDED Requirements

### Requirement: Plugin History Retains Its Written Definition
Plugin-owned persistent history SHALL remain readable using its stored definition after a plugin changes or disappears, without resetting storage or converting old entries into current state.

#### Scenario: Plugin Shape Changes
- **WHEN** plugin record, fact, object or stream history exists and the plugin changes shape
- **THEN** root opening succeeds and historical reads use the stored definition; incompatible typed current reads explicitly refuse

#### Scenario: Plugin Is Absent
- **WHEN** a root is reopened without the plugin or its object definition
- **THEN** its history remains stored and readable without current plugin decoders, and background retention preserves absent definitions

### Requirement: Host Shape Refusal Remains
Host built-in state record and transaction catalog changes SHALL retain the existing named incompatibility refusal.

#### Scenario: Host Record Changes
- **WHEN** a host record in the journal or snapshot inventory has a different current revision
- **THEN** opening refuses with the stored identity and fresh-root guidance and preserves history
