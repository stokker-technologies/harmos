# Design

## Contract

```rust
#[harmos::message]
struct Labels {
    values: std::collections::HashMap<String, String>,
}
```

HashMap and BTreeMap share a length-prefixed sequence of key/value entries.
Keys encode in ascending Rust order; supported keys are bool, u32, u64, i32,
i64 and String. Values are supported scalars, bytes or named messages.
Entry schemas have required key/tag 1 and value/tag 2; a root map has an
entries/tag 1 repeated entry field. Both containers share the same schema.
Nested containers remain unsupported; custom HashMap hashers supply MemorySize accounting for their default state.
Duplicate keys are refused during typed decoding. Input entry order may vary;
subsequent encoding restores canonical key order.
Finite payload validation and gRPC transcode preserve the repeated-entry shape.

## Ownership

One worktree, feat/message-map-encoding, starts from remote dev.
Domain: message/maps.rs, message/mod.rs, schema.rs, private exports.
Macros: message.rs recognition and field diagnostics.
Tests: existing message and transport test owners plus focused map stories.
README and the message guide describe wire representation and constraints.
Consumer integration uses local overrides; publication requires separate authorization.
No consumer-private source, identifiers or paths enter upstream files.

## Compatibility

Previously rejected map types gain support. Existing messages retain their bytes
and fingerprints. Maps with equal entries have equal bytes and fingerprints
across insertion orders, hash seeds and the two supported container types.
Memory accounting includes table/node allocations and recursively owned values.
No application, instrument, hardware or product suite is executed by this work.

## Consumer Adoption

Use this development worktree as the local path override; do not backport to
the published API. Merge newer remote dev changes into this worktree.
Migrate affected consumer assembly and dependent APIs to the selected tree.

Root Option<T> uses an optional value/tag 1 field with T's scalar or message
kind, preserving None versus Some(default) through Postcard and protobuf.
Existing nested optional fields keep their current encoding.

## Runtime Triage

All source tasks already landed in `a9f77943`; `harmos-domain/tests/message/maps.rs` covers canonical maps, refusal, accounting and root optional presence. Final required validation (task 3.2) stays with the integration owner; no map implementation is repeated here.
