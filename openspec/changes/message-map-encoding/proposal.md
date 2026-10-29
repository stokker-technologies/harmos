# Message Map Encoding

## Why

Portable messages currently reject standard Rust maps, forcing callers to replace useful key/value APIs with authored entry containers.

## Changes

Support HashMap and BTreeMap as message fields and complete operation payloads.
Reuse finite repeated key/value entry schemas, deterministic Postcard bytes,
existing protobuf repeated-message transport, and existing admission boundaries.
Do not introduce another schema kind, native protobuf map syntax, or JSON-object projection.
Retain existing encodings for all previously supported types.

## Impact

The domain owns map codecs, schema definitions and memory accounting.
Macros recognize map fields. Existing schema and transport consumers use entry
messages without new branches. Documentation and focused codec/transport tests
explain and verify that representation.
