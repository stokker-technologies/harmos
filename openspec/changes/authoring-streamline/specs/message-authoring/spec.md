## ADDED Requirements

### Requirement: Message Identity

Portable messages SHALL expose type-level fingerprint() for complete canonical schema identity and value-level digest() for canonical contents combined with schema identity. Algorithms and formats SHALL be stable across processes and supported languages, with explicit floating-point semantics and support for nested and recursive schemas.

#### Scenario: Type And Value Identity

- **WHEN** two values share a message type but differ in canonical encoded contents
- **THEN** they share a schema fingerprint and their content-digest inputs differ.

#### Scenario: Nested Schema Change

- **WHEN** a nested message schema changes
- **THEN** its enclosing message fingerprint reflects the changed schema.

### Requirement: Encoding And Sizing

Messages SHALL expose encode(), decode(bytes), encode_into(buffer), encoded_size() and memory_size(). Encoded size SHALL measure canonical serialized bytes; memory size SHALL preserve inline and owned-allocation accounting including capacity. Decoding SHALL retain memory-admission safeguards.

#### Scenario: Spare Allocation Capacity

- **WHEN** equivalent message contents have different owned allocation capacities
- **THEN** encoded size and content digest remain the same while memory accounting includes those capacities.

#### Scenario: Encoding Round Trip

- **WHEN** supported message contents are encoded and decoded
- **THEN** decoding preserves the defined value/bit-pattern representation, and encoded_size matches the bytes appended by encoding.

### Requirement: Ordinary Rust Derivation

Message and operation macros SHALL preserve ordinary derive attributes and existing Debug customization without duplicate implementations. Clone, PartialEq, Eq, standard Hash and Default SHALL remain optional subject to field support.

#### Scenario: Floating-Point Message

- **WHEN** a supported floating-point message derives PartialEq without Eq or standard Hash
- **THEN** message encoding, schema fingerprinting and content digest remain available.
