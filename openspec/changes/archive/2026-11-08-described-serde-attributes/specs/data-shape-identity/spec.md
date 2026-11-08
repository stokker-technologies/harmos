## ADDED Requirements

### Requirement: Stored Serde Wire Forms
Described SHALL describe the serialization form of supported Serde tagging, naming and omission attributes. Unsupported attributes SHALL fail at compile time naming the attribute and enclosing type. Deserialization-only attributes SHALL NOT affect identity. Revisions and fingerprints SHALL share one canonical encoding, preserving existing golden vectors for unchanged schemas.

#### Scenario: Supported Form
- **WHEN** a supported attributed type is serialized by Serde
- **THEN** its derived schema encodes and decodes the same structural value, including missing and null fields

#### Scenario: Unsupported Form
- **WHEN** a type uses a custom serialization or unsupported flattening attribute
- **THEN** deriving fails with the attribute and type in the diagnostic

#### Scenario: Identity
- **WHEN** a serialization name, enum tagging or omission rule changes
- **THEN** the derived shape revision changes
