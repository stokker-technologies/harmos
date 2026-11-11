# Schema-Derived Stream Revisions

Plugin stream rows currently share an authored ordinal fixed at one. Changed rows must automatically receive stable revisions so retained rows remain decodable across reloads and restarts.

The owner has locked automatic revisions derived in the core from canonical row schemas and windows. Presentation changes preserve identity. Current selection follows committed activation, never numeric hash order. Retired live windows are released, existing readers retain descriptors, and staging remains atomic. Remove obsolete author version constants, refusal codes and restart advice. Native streams retain their required contract. Wire and storage formats remain at v1; development surface replacements migrate all repository consumers.


The implementation removes plugin-authored schema ordinals from inventories,
listener stream sources and the `host.publish` payload. The activation already
owns the immutable id-to-row inventory. The host derives the persisted revision
at `publication_definitions`; SDKs carry only the stream id. Native stream
versions retain their existing storage contract. The scalar revision key and
schema/storage format versions stay unchanged at v1. Current lookup is explicit;
reader handles remain pinned, and live feeds carry per-row descriptors.

Historical collision evidence retains only owner, window and a canonical row
fingerprint. Retired windows and schema descriptors are released after attached
recordings and live feeds have taken their rows; existing readers independently
retain their own descriptors. This checkout has C++ file skeletons but no C++
prototype or SDK source to migrate. The SDK compile check covers the Rust carrier
surfaces and Schematic consumers present in this repository.
