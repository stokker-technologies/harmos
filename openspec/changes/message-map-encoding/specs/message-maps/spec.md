# Message Maps

## ADDED Requirements

### Requirement: Portable Standard Maps

Messages SHALL support HashMap and BTreeMap fields and root payloads with
boolean, supported integer or string keys and supported scalar, bytes or message values.

#### Scenario: Field And Root Maps

- **WHEN** an operation exchanges maps as a field or its complete payload
- **THEN** declaration, schema preparation, Postcard and protobuf transcode succeed

### Requirement: Deterministic Map Bytes

Map encoding SHALL order keys and produce the same bytes and fingerprints
for equal contents across supported containers and insertion orders.

#### Scenario: Different Hash Seeds And Insertion Orders

- **WHEN** equal entries are inserted in different orders into both containers
- **THEN** encoded bytes, sizes, fingerprints and content digests agree

### Requirement: Bounded Map Decoding

Map decoding SHALL obey existing structural work and nesting limits,
account conservatively for container and nested allocations, and reject
duplicate keys, overflow and malformed/truncated payloads.

#### Scenario: Invalid Or Excessive Payload

- **WHEN** a payload has invalid keys, lengths, ordering or nesting
- **THEN** typed decoding refuses it without unchecked allocation growth

### Requirement: Repeated Entry Schema

Maps SHALL use existing repeated entry-message schemas with key/tag 1 and
value/tag 2; root maps SHALL wrap entries at tag 1.

#### Scenario: Portable Transport Roundtrip

- **WHEN** a map schema is exported and transcoded through protobuf
- **THEN** its repeated entry representation roundtrips to the same typed map


### Requirement: Root Optional Payloads

Option<T> SHALL be portable as a complete operation payload when T has a
portable schema, preserving absence independently from the contained default.

#### Scenario: Present Default And Absent Values

- **WHEN** None and Some(default) cross Postcard and protobuf boundaries
- **THEN** both roundtrip with their original presence and distinct digests
